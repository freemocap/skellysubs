use super::*;
use crate::ai::graph::Disposition;

#[test]
fn native_help_command_demands_exact_source_atomically_and_replays_without_legacy_rows() {
    let (dir, mut store, conversation) = setup();
    store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution.reading','on_demand')", []).unwrap();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    let message: String = store
        .connection
        .query_row(
            "SELECT id FROM messages WHERE turn_id=?1 AND role='user'",
            [&turn],
            |r| r.get(0),
        )
        .unwrap();
    let before = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(
        before.nodes["learner_translation"],
        Disposition::Unrequested
    );
    let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
    let surfaces = snapshot
        .turns
        .iter()
        .find(|view| view.id == turn)
        .unwrap()
        .native_surfaces
        .as_ref()
        .unwrap();
    assert_eq!(surfaces["text"], ["text"]);
    assert_eq!(surfaces["learner.translation"], ["learner_translation"]);
    assert_eq!(surfaces["reply.translation"], ["reply_translation"]);
    assert_eq!(surfaces["reply_brief"], ["brief"]);
    assert_eq!(surfaces["reply_assistance"], ["assistance"]);
    assert_eq!(surfaces["reply_explanations"], ["explanations"]);
    assert_eq!(surfaces["coach_feedback"], ["feedback"]);
    assert_eq!(surfaces["skill_assessment"], ["assessment"]);
    // Merely exposing associations must leave on-demand work dormant.
    assert_eq!(
        snapshot.turns[0].native_graph.as_ref().unwrap().nodes,
        before.nodes
    );
    // Reply, assessment, attribution, feedback and brief reserve work. Dormant
    // reading, grammar, suggestions and read-aloud branches do not.
    assert_eq!(
        graph_runtime::outstanding(&store.connection, &store.graph_runtime).unwrap(),
        5
    );
    let command = Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::RequestMessageHelp {
            message_id: message.clone(),
            help: MessageHelp::Translation,
            retry: false,
        },
    };
    // Reject the native event write after the command receipt has been staged.
    store.connection.execute_batch("CREATE TEMP TRIGGER reject_help BEFORE UPDATE ON graph_engines BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;").unwrap();
    assert!(store.execute(command.clone()).is_err());
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM receipts WHERE action_id=?1",
                [&command.action_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let failed = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(failed.nodes, before.nodes);
    store
        .connection
        .execute_batch("DROP TRIGGER reject_help")
        .unwrap();
    let receipt = store.execute(command.clone()).unwrap();
    assert!(receipt.entity_id.starts_with("graph:"));
    let after = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    // Demand is retained while the declared context prerequisite is unfinished.
    assert_eq!(after.nodes["learner_translation"], Disposition::Waiting);
    assert_eq!(after.nodes["learner_gloss"], Disposition::Unrequested);
    assert_eq!(
        graph_runtime::outstanding(&store.connection, &store.graph_runtime).unwrap(),
        6
    );
    apply(
        &mut store,
        Action::ControlTurn {
            turn_id: turn.clone(),
            control: TurnControl::Pause,
        },
    );
    assert_eq!(
        graph_runtime::outstanding(&store.connection, &store.graph_runtime).unwrap(),
        6
    );
    assert_eq!(after.artifact_id, before.artifact_id);
    assert_eq!(store.execute(command).unwrap().entity_id, receipt.entity_id);
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM operations WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    drop(store);
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    let recovered = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.nodes["learner_translation"], Disposition::Waiting);
    store
        .connection
        .execute("UPDATE turns SET state='invalidated' WHERE id=?1", [&turn])
        .unwrap();
    let invalid = Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::RequestMessageHelp {
            message_id: message,
            help: MessageHelp::WordGloss,
            retry: false,
        },
    };
    assert!(store.execute(invalid).is_err());
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM operations WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
