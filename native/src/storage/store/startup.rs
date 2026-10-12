use super::*;

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let ownership = WorkspaceOwnership::acquire(path)?;
        migrations::validate_registry()?;
        let config = crate::configuration::Registry::bundled()
            .map_err(|e| AppError::new(ErrorCode::ConfigLoad, e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let private_file = || -> std::io::Result<()> {
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .mode(0o600)
                    .open(path)?;
                file.set_permissions(std::fs::Permissions::from_mode(0o600))
            };
            private_file().map_err(|cause| {
                crate::diagnostics::response::io_context(
                    &cause,
                    "workspace_file_permissions",
                    AppError::new(
                        ErrorCode::Storage,
                        "Could not restrict database access to this user.",
                    ),
                )
            })?;
        }
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.busy_timeout(std::time::Duration::from_secs(3))?;
        let version: i32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == 0 {
            let count: i32 = connection.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            if count != 0 {
                return Err(AppError::new(
                    ErrorCode::Storage,
                    "Unrecognized database. No data was changed. Use Factory Reset to start a new workspace.",
                ));
            }
            // Pending audio is stored transactionally in SQLite. Reclaim its
            // freed pages after pruning rather than retaining peak disk usage.
            connection.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
            let tx = connection.transaction()?;
            tx.execute_batch(include_str!("../schemas/schema.sql"))?;
            tx.execute_batch(GENERATION_SCHEMA)?;
            schema::create_extensions(&tx)?;
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            config.language("english")?;
            let preferences = Preferences {
                execution: Default::default(),
                practice_view: None,
                theme: Theme::Light,
                appearance: Default::default(),
                explanation_language: "english".into(),
                explanation_variety_id: config.language("english")?.default_variety,
                interface_locale: "english".into(),
                target_varieties: Default::default(),
                script_scales: None,
                my_languages: Vec::new(),
                text_size: crate::model::TEXT_SIZE_DEFAULT,
                text_spacing: 0,
                high_contrast: false,
                onboarding: OnboardingStatus::NotStarted,
                onboarding_required: true,
                onboarding_language: None,
                onboarding_help: false,
            };
            tx.execute(
                "INSERT INTO learner VALUES(?1,1,'Learner',1,?2)",
                params![id(), serde_json::to_string(&preferences)?],
            )?;
            tx.commit()?;
        }
        migrations::upgrade(&mut connection, path, &config)?;
        validate_database(&connection)?;
        validate_current_schema(&connection)?;
        crate::conversations::execution_owner::validate_complete(&connection)?;
        crate::language::reading::preloaded::install_bundled(&mut connection, &config)?;
        let mut store = Self {
            workspace_graphs: Default::default(),
            graph_runtime: crate::conversations::execution::graph_runtime::Runtime::new()?,
            config,
            connection,
            session_id: id(),
            audio_analysis: Default::default(),
            credential_writes: std::collections::HashSet::new(),
            credential_index: path.with_file_name("credentials.index"),
            drill_audio: path.with_file_name("drill-audio"),
            ownership,
        };
        store.snapshot()?;
        crate::ai::results::recover(&store.connection)?;
        store.graph_runtime.recover(&mut store.connection)?;
        crate::drill::sessions::recover(&store.connection)?;
        crate::ai::generation::generation_receipts::recover(&store.connection)?;
        crate::language::reading::recover(&store.connection)?;
        crate::speech::recording::transcription::recover(&store.connection)?;
        // Audio an interrupted save or deletion left with no attempt to claim it.
        store.reconcile_drill_audio()?;
        store.prune_drill_audio()?;
        store.connection.execute("DELETE FROM receipts", [])?;
        Ok(store)
    }

    pub fn prepare_chat(&mut self) -> Result<()> {
        let snapshot = self.snapshot()?;
        if snapshot.conversations.iter().any(|c| {
            !c.archived
                && snapshot
                    .contacts
                    .iter()
                    .any(|r| r.id == c.contact_id && !r.archived)
        }) {
            return Ok(());
        }
        let action = match snapshot.contacts.iter().find(|r| !r.archived) {
            Some(contact) => Action::CreateConversation {
                contact_id: contact.id.clone(),
                title: "New conversation".into(),
            },
            None => Action::StartChat {
                language_id: "spanish".into(),
            },
        };
        self.execute(Command {
            session_id: self.session_id.clone(),
            action_id: id(),
            action,
        })?;
        Ok(())
    }
}
