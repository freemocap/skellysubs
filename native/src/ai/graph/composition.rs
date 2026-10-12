use super::{model::fault, registry::valid_name, *};
use std::collections::BTreeMap;

impl Definition {
    /// Composition captures the child's source and verified identity. Compilation
    /// resolves and expands it, rather than trusting separately editable metadata.
    pub fn compose(
        &mut self,
        prefix: &str,
        child: &Executable,
        inputs: BTreeMap<String, Source>,
    ) -> Result<BTreeMap<String, Source>> {
        valid_name(prefix)?;
        if self.compositions.contains_key(prefix) {
            return Err(fault(CoreFaultCode::DuplicateComposition, prefix));
        }
        if child.source.inputs.keys().ne(inputs.keys()) {
            return Err(fault(CoreFaultCode::BindingSet, prefix));
        }
        let results = child
            .plan
            .artifact
            .definition
            .results
            .iter()
            .map(|(port, source)| Ok((port.clone(), qualify(prefix, source, &inputs)?)))
            .collect::<Result<_>>()?;
        self.compositions.insert(
            prefix.into(),
            Boundary {
                artifact: child.plan.identity.clone(),
                source: Box::new(child.source.clone()),
                bindings: inputs,
            },
        );
        Ok(results)
    }
}

pub(super) fn qualify(
    prefix: &str,
    source: &Source,
    inputs: &BTreeMap<String, Source>,
) -> Result<Source> {
    Ok(match source {
        Source::Input(name) => inputs
            .get(name)
            .cloned()
            .ok_or_else(|| fault(CoreFaultCode::UnknownInput, name))?,
        Source::Output { node, port } => Source::Output {
            node: format!("{prefix}/{node}"),
            port: port.clone(),
        },
        other => other.clone(),
    })
}

impl Registry {
    pub(super) fn expand(&self, definition: &mut Definition, depth: usize) -> Result<()> {
        if depth > 16 {
            return Err(fault(CoreFaultCode::CompositionDepth, "graph"));
        }
        for (prefix, boundary) in &definition.compositions {
            valid_name(prefix)?;
            let child = self.compile_at(*boundary.source.clone(), depth + 1)?;
            if child.plan.identity != boundary.artifact {
                return Err(fault(CoreFaultCode::CompositionChanged, prefix));
            }
            if child.source.inputs.keys().ne(boundary.bindings.keys()) {
                return Err(fault(CoreFaultCode::BindingSet, prefix));
            }
            for (name, node) in child.plan.artifact.definition.nodes.clone() {
                let id = format!("{prefix}/{name}");
                if definition.nodes.contains_key(&id) {
                    return Err(fault(CoreFaultCode::DuplicateNode, &id));
                }
                let inputs = node
                    .inputs
                    .iter()
                    .map(|(k, s)| Ok((k.clone(), qualify(prefix, s, &boundary.bindings)?)))
                    .collect::<Result<_>>()?;
                let guard = node
                    .guard
                    .as_ref()
                    .map(|s| qualify(prefix, s, &boundary.bindings))
                    .transpose()?;
                let after = node.after.iter().map(|p| format!("{prefix}/{p}")).collect();
                definition.nodes.insert(
                    id,
                    Node {
                        inputs,
                        guard,
                        after,
                        ..node
                    },
                );
            }
        }
        Ok(())
    }
}
