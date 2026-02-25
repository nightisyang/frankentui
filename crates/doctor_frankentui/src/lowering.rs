//! Lowering pipeline: extracted semantic facts → canonical [`MigrationIr`].
//!
//! Consumes the output of:
//! - [`composition_semantics::extract_composition_semantics`] → view tree
//! - [`style_semantics::extract_style_semantics`] → style intent + design tokens
//! - [`state_effects::build_project_state_model`] → state graph, effects, capabilities
//!
//! Produces a fully-validated [`MigrationIr`] via [`IrBuilder`] with:
//! - Deterministic symbol resolution and stable node identities
//! - Complete provenance back to source locations
//! - Structured diagnostics for unresolved or partial constructs

use std::collections::{BTreeMap, BTreeSet};

use crate::composition_semantics::{self, CompositionSemanticsResult};
use crate::migration_ir::{
    self, AccessibilityEntry, DerivedState, EffectDecl, EffectKind, EventDecl, EventKind,
    EventTransition, IrBuilder, IrNodeId, IrWarning, MigrationIr, Provenance, StateScope,
    StateVariable,
};
use crate::state_effects::{
    self, EffectClassification, EventStateTransition, ProjectStateModel, StateVarScope,
};
use crate::style_semantics::{self, StyleSemanticsResult, StyleWarningKind};
use crate::tsx_parser::ProjectParse;

// ── Public API ──────────────────────────────────────────────────────────

/// Configuration for the lowering pipeline.
#[derive(Debug, Clone)]
pub struct LoweringConfig {
    /// Identifier for this lowering run.
    pub run_id: String,
    /// Source project name or path.
    pub source_project: String,
}

/// Result of the lowering pipeline.
#[derive(Debug)]
pub struct LoweringResult {
    /// The canonical IR.
    pub ir: MigrationIr,
    /// Diagnostics emitted during lowering (non-fatal).
    pub diagnostics: Vec<LoweringDiagnostic>,
}

/// A structured diagnostic from the lowering process.
#[derive(Debug, Clone)]
pub struct LoweringDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub provenance: Option<Provenance>,
}

/// Severity of a lowering diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Warning,
    Info,
}

/// Lower all extracted semantic facts into a canonical `MigrationIr`.
///
/// This is the main entry point for the lowering pipeline.
pub fn lower_to_ir(
    config: &LoweringConfig,
    project: &ProjectParse,
    composition: &CompositionSemanticsResult,
    styles: &StyleSemanticsResult,
    state_model: &ProjectStateModel,
) -> LoweringResult {
    let mut builder = IrBuilder::new(config.run_id.clone(), config.source_project.clone());
    let mut diagnostics = Vec::new();

    builder.set_source_file_count(project.files.len());

    // Phase 1: View tree (composition semantics → ViewTree)
    lower_view_tree(&mut builder, composition, &mut diagnostics);

    // Phase 2: State graph (state effects → StateGraph)
    let state_id_map = lower_state_graph(&mut builder, state_model, &mut diagnostics);

    // Phase 3: Events and transitions
    lower_events(&mut builder, state_model, &state_id_map, &mut diagnostics);

    // Phase 4: Effects
    lower_effects(&mut builder, state_model, &state_id_map, &mut diagnostics);

    // Phase 5: Style intent (style semantics → StyleIntent)
    lower_style_intent(&mut builder, styles, &mut diagnostics);

    // Phase 6: Capabilities
    lower_capabilities(&mut builder, state_model);

    // Phase 7: Accessibility (from style + composition hints)
    lower_accessibility(&mut builder, styles, composition, &mut diagnostics);

    // Phase 8: Propagate warnings from extraction layers
    propagate_extraction_warnings(&mut builder, composition, styles, &mut diagnostics);

    let ir = builder.build();

    LoweringResult { ir, diagnostics }
}

/// Lower from a `ProjectParse` by running all extraction phases first.
///
/// Convenience function that chains extraction → lowering.
pub fn lower_project(config: &LoweringConfig, project: &ProjectParse) -> LoweringResult {
    let composition = composition_semantics::extract_composition_semantics(project);
    let styles = style_semantics::extract_style_semantics(project);
    // state_effects requires file contents for global store detection; use empty map for now.
    let empty_contents = BTreeMap::new();
    let state_model = state_effects::build_project_state_model(&project.files, &empty_contents);

    lower_to_ir(config, project, &composition, &styles, &state_model)
}

// ── Phase 1: View Tree ──────────────────────────────────────────────────

fn lower_view_tree(
    builder: &mut IrBuilder,
    composition: &CompositionSemanticsResult,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let view_tree = composition_semantics::to_view_tree(composition);

    for root_id in &view_tree.roots {
        builder.add_root(root_id.clone());
    }

    for (id, node) in &view_tree.nodes {
        builder.add_view_node(node.clone());

        // Emit diagnostic for nodes with no provenance file info.
        if node.provenance.file.is_empty() {
            diagnostics.push(LoweringDiagnostic {
                code: "L001".to_string(),
                severity: DiagnosticSeverity::Warning,
                message: format!(
                    "View node '{name}' ({id}) has empty provenance file",
                    name = node.name
                ),
                provenance: Some(node.provenance.clone()),
            });
        }
    }

    if view_tree.roots.is_empty() && !composition.component_tree.nodes.is_empty() {
        diagnostics.push(LoweringDiagnostic {
            code: "L002".to_string(),
            severity: DiagnosticSeverity::Info,
            message: format!(
                "View tree has {} nodes but no roots — all components may be non-root",
                composition.component_tree.nodes.len()
            ),
            provenance: None,
        });
    }
}

// ── Phase 2: State Graph ────────────────────────────────────────────────

/// Maps `(component_name, variable_name)` → `IrNodeId` for cross-reference.
type StateIdMap = BTreeMap<(String, String), IrNodeId>;

fn lower_state_graph(
    builder: &mut IrBuilder,
    state_model: &ProjectStateModel,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) -> StateIdMap {
    let mut state_id_map = StateIdMap::new();

    for (comp_name, comp_model) in &state_model.components {
        // State variables
        for var in &comp_model.state_vars {
            let id_content = format!("state:{}:{}:{}", comp_model.file, comp_name, var.name);
            let id = migration_ir::make_node_id(id_content.as_bytes());

            state_id_map.insert((comp_name.clone(), var.name.clone()), id.clone());

            // Also map setter name for lookup from event transitions.
            if let Some(setter) = &var.setter {
                state_id_map.insert((comp_name.clone(), setter.clone()), id.clone());
            }

            let scope = map_state_scope(&var.scope);

            builder.add_state_variable(StateVariable {
                id: id.clone(),
                name: var.name.clone(),
                scope,
                type_annotation: var.type_hint.clone(),
                initial_value: var.initial_value.clone(),
                readers: BTreeSet::new(),
                writers: BTreeSet::new(),
                provenance: Provenance {
                    file: comp_model.file.clone(),
                    line: var.line,
                    column: None,
                    source_name: Some(format!("{}::{}", comp_name, var.name)),
                    policy_category: Some("state".to_string()),
                },
            });
        }

        // Derived state (useMemo, useCallback)
        for derived in &comp_model.derived {
            let name = derived.name.as_deref().unwrap_or("anonymous_derived");
            let id_content = format!("derived:{}:{}:{}", comp_model.file, comp_name, name);
            let id = migration_ir::make_node_id(id_content.as_bytes());

            // Resolve dependency IDs.
            let dep_ids: BTreeSet<IrNodeId> = derived
                .dependencies
                .iter()
                .filter_map(|dep_name| {
                    state_id_map
                        .get(&(comp_name.clone(), dep_name.clone()))
                        .cloned()
                })
                .collect();

            if dep_ids.len() < derived.dependencies.len() {
                let unresolved: Vec<_> = derived
                    .dependencies
                    .iter()
                    .filter(|d| !state_id_map.contains_key(&(comp_name.clone(), (*d).clone())))
                    .collect();
                diagnostics.push(LoweringDiagnostic {
                    code: "L010".to_string(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!(
                        "Derived computation '{}' in {} has unresolved deps: {:?}",
                        name, comp_name, unresolved
                    ),
                    provenance: Some(Provenance {
                        file: comp_model.file.clone(),
                        line: derived.line,
                        column: None,
                        source_name: Some(format!("{}::{}", comp_name, name)),
                        policy_category: Some("state".to_string()),
                    }),
                });
            }

            builder.add_derived_state(DerivedState {
                id,
                name: name.to_string(),
                dependencies: dep_ids,
                expression_snippet: derived.expression_snippet.clone(),
                provenance: Provenance {
                    file: comp_model.file.clone(),
                    line: derived.line,
                    column: None,
                    source_name: Some(format!("{}::{}", comp_name, name)),
                    policy_category: Some("derived".to_string()),
                },
            });
        }

        // Context provider → consumer data flow edges.
    }

    // Add data flow edges from context graph.
    for edge in &state_model.context_graph {
        // Provider→consumer is an implicit data flow via context.
        let provider_key = (edge.provider_component.clone(), edge.context_name.clone());
        let consumer_key = (edge.consumer_component.clone(), edge.context_name.clone());

        if let (Some(from_id), Some(to_id)) = (
            state_id_map.get(&provider_key),
            state_id_map.get(&consumer_key),
        ) {
            builder.add_data_flow(from_id.clone(), to_id.clone());
        }
    }

    state_id_map
}

fn map_state_scope(scope: &StateVarScope) -> StateScope {
    match scope {
        StateVarScope::Local => StateScope::Local,
        StateVarScope::Reducer => StateScope::Local,
        StateVarScope::Ref => StateScope::Local,
        StateVarScope::Context => StateScope::Context,
        StateVarScope::ExternalStore => StateScope::Global,
        StateVarScope::Url => StateScope::Route,
        StateVarScope::Server => StateScope::Server,
    }
}

// ── Phase 3: Events ─────────────────────────────────────────────────────

fn lower_events(
    builder: &mut IrBuilder,
    state_model: &ProjectStateModel,
    state_id_map: &StateIdMap,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    for (comp_name, comp_model) in &state_model.components {
        for transition in &comp_model.event_transitions {
            let event_id = make_event_id(&comp_model.file, comp_name, transition);

            let kind = classify_event_kind(&transition.event_name);

            builder.add_event(EventDecl {
                id: event_id.clone(),
                name: transition.event_name.clone(),
                kind,
                source_node: None,
                payload_type: None,
                provenance: Provenance {
                    file: comp_model.file.clone(),
                    line: transition.line,
                    column: None,
                    source_name: transition.handler_name.clone(),
                    policy_category: Some("event".to_string()),
                },
            });

            // Create transitions for each state write.
            for state_write in &transition.state_writes {
                let target_id = state_id_map.get(&(comp_name.clone(), state_write.clone()));

                if let Some(target) = target_id {
                    builder.add_transition(EventTransition {
                        event_id: event_id.clone(),
                        target_state: target.clone(),
                        action_snippet: format!(
                            "{}({})",
                            state_write,
                            transition.handler_name.as_deref().unwrap_or("handler")
                        ),
                        guards: Vec::new(),
                    });
                } else {
                    diagnostics.push(LoweringDiagnostic {
                        code: "L020".to_string(),
                        severity: DiagnosticSeverity::Warning,
                        message: format!(
                            "Event '{}' in {} writes to '{}' but target state not found",
                            transition.event_name, comp_name, state_write
                        ),
                        provenance: Some(Provenance {
                            file: comp_model.file.clone(),
                            line: transition.line,
                            column: None,
                            source_name: transition.handler_name.clone(),
                            policy_category: Some("event".to_string()),
                        }),
                    });
                }
            }
        }
    }
}

fn make_event_id(file: &str, comp_name: &str, transition: &EventStateTransition) -> IrNodeId {
    let content = format!(
        "event:{}:{}:{}:{}",
        file, comp_name, transition.event_name, transition.line
    );
    migration_ir::make_node_id(content.as_bytes())
}

fn classify_event_kind(event_name: &str) -> EventKind {
    let name = event_name.to_lowercase();
    let is_user_input = name.strip_prefix("on").is_some_and(|suffix| {
        suffix.starts_with("click")
            || suffix.starts_with("mouse")
            || suffix.starts_with("key")
            || suffix.starts_with("touch")
            || suffix.starts_with("pointer")
            || suffix.starts_with("drag")
            || suffix.starts_with("drop")
            || suffix.starts_with("input")
            || suffix.starts_with("change")
            || suffix.starts_with("submit")
            || suffix.starts_with("focus")
            || suffix.starts_with("blur")
            || suffix.starts_with("scroll")
    });
    if is_user_input {
        return EventKind::UserInput;
    }

    if name.contains("mount") || name.contains("unmount") || name.contains("update") {
        return EventKind::Lifecycle;
    }

    if name.contains("timer") || name.contains("interval") || name.contains("timeout") {
        return EventKind::Timer;
    }

    if name.contains("fetch") || name.contains("response") || name.contains("request") {
        return EventKind::Network;
    }

    EventKind::Custom
}

// ── Phase 4: Effects ────────────────────────────────────────────────────

fn lower_effects(
    builder: &mut IrBuilder,
    state_model: &ProjectStateModel,
    state_id_map: &StateIdMap,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    for (comp_name, comp_model) in &state_model.components {
        for (idx, effect) in comp_model.effects.iter().enumerate() {
            let id_content = format!(
                "effect:{}:{}:{}:{}",
                comp_model.file, comp_name, effect.hook, effect.line
            );
            let id = migration_ir::make_node_id(id_content.as_bytes());

            let kind = map_effect_kind(&effect.kind);

            // Resolve dependency IDs.
            let dep_ids: BTreeSet<IrNodeId> = effect
                .dependencies
                .iter()
                .filter_map(|dep| state_id_map.get(&(comp_name.clone(), dep.clone())).cloned())
                .collect();

            let read_ids: BTreeSet<IrNodeId> = effect
                .reads
                .iter()
                .filter_map(|r| state_id_map.get(&(comp_name.clone(), r.clone())).cloned())
                .collect();

            let write_ids: BTreeSet<IrNodeId> = effect
                .writes
                .iter()
                .filter_map(|w| state_id_map.get(&(comp_name.clone(), w.clone())).cloned())
                .collect();

            // Emit diagnostic for unresolved reads/writes.
            let total_refs = effect.reads.len() + effect.writes.len();
            let resolved_refs = read_ids.len() + write_ids.len();
            if resolved_refs < total_refs {
                diagnostics.push(LoweringDiagnostic {
                    code: "L030".to_string(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!(
                        "Effect #{} ({}) in {} has {} unresolved state references",
                        idx,
                        effect.hook,
                        comp_name,
                        total_refs - resolved_refs
                    ),
                    provenance: Some(Provenance {
                        file: comp_model.file.clone(),
                        line: effect.line,
                        column: None,
                        source_name: Some(format!("{}::effect#{}", comp_name, idx)),
                        policy_category: Some("effect".to_string()),
                    }),
                });
            }

            builder.add_effect(EffectDecl {
                id,
                name: format!("{}::effect#{}", comp_name, idx),
                kind,
                dependencies: dep_ids,
                has_cleanup: effect.has_cleanup,
                reads: read_ids,
                writes: write_ids,
                provenance: Provenance {
                    file: comp_model.file.clone(),
                    line: effect.line,
                    column: None,
                    source_name: Some(format!("{}::{}", comp_name, effect.hook)),
                    policy_category: Some("effect".to_string()),
                },
            });
        }
    }
}

fn map_effect_kind(classification: &EffectClassification) -> EffectKind {
    match classification {
        EffectClassification::DataFetch => EffectKind::Network,
        EffectClassification::DomManipulation => EffectKind::Dom,
        EffectClassification::EventListener => EffectKind::Subscription,
        EffectClassification::Timer => EffectKind::Timer,
        EffectClassification::Subscription => EffectKind::Subscription,
        EffectClassification::Sync => EffectKind::Storage,
        EffectClassification::Telemetry => EffectKind::Telemetry,
        EffectClassification::Unknown => EffectKind::Other,
    }
}

// ── Phase 5: Style Intent ───────────────────────────────────────────────

fn lower_style_intent(
    builder: &mut IrBuilder,
    styles: &StyleSemanticsResult,
    _diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let intent = style_semantics::to_style_intent(styles);

    for token in intent.tokens.values() {
        builder.add_style_token(token.clone());
    }

    for (node_id, layout) in &intent.layouts {
        builder.add_layout(node_id.clone(), layout.clone());
    }

    for theme in &intent.themes {
        builder.add_theme(theme.clone());
    }
}

// ── Phase 6: Capabilities ───────────────────────────────────────────────

fn lower_capabilities(builder: &mut IrBuilder, state_model: &ProjectStateModel) {
    for cap in &state_model.required_capabilities {
        builder.require_capability(cap.clone());
    }

    for cap in &state_model.optional_capabilities {
        builder.optional_capability(cap.clone());
    }

    for assumption in &state_model.platform_assumptions {
        builder.add_platform_assumption(assumption.clone());
    }
}

// ── Phase 7: Accessibility ──────────────────────────────────────────────

fn lower_accessibility(
    builder: &mut IrBuilder,
    styles: &StyleSemanticsResult,
    composition: &CompositionSemanticsResult,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let a11y_meta = style_semantics::accessibility_meta(styles);

    // Add entries for components with accessibility-relevant style properties.
    for id in &a11y_meta.components_with_colors {
        builder.add_accessibility(AccessibilityEntry {
            node_id: id.clone(),
            role: None,
            label: None,
            description: Some("Has explicit color declarations — verify contrast".to_string()),
            keyboard_shortcut: None,
            focus_order: None,
            live_region: None,
        });
    }

    // Check composition tree for interactive elements without accessibility hints.
    for (id, node) in &composition.component_tree.nodes {
        let has_interactive_props = node.props_contract.iter().any(|p| p.is_callback);
        if has_interactive_props {
            builder.add_accessibility(AccessibilityEntry {
                node_id: id.clone(),
                role: Some("interactive".to_string()),
                label: None,
                description: Some(format!(
                    "Component '{}' has callback props — may need keyboard support",
                    node.component_name
                )),
                keyboard_shortcut: None,
                focus_order: None,
                live_region: None,
            });

            diagnostics.push(LoweringDiagnostic {
                code: "L040".to_string(),
                severity: DiagnosticSeverity::Info,
                message: format!(
                    "Interactive component '{}' should define ARIA role and keyboard handling",
                    node.component_name
                ),
                provenance: Some(Provenance {
                    file: node.source_file.clone(),
                    line: node.line_start,
                    column: None,
                    source_name: Some(node.component_name.clone()),
                    policy_category: Some("accessibility".to_string()),
                }),
            });
        }
    }
}

// ── Phase 8: Warning propagation ────────────────────────────────────────

fn propagate_extraction_warnings(
    builder: &mut IrBuilder,
    composition: &CompositionSemanticsResult,
    styles: &StyleSemanticsResult,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    // Composition warnings
    for w in &composition.warnings {
        builder.add_warning(IrWarning {
            code: format!("CS-{}", w.code),
            message: w.message.clone(),
            provenance: Some(Provenance {
                file: w.file.clone(),
                line: w.line.unwrap_or(0),
                column: None,
                source_name: None,
                policy_category: Some("composition".to_string()),
            }),
        });
        diagnostics.push(LoweringDiagnostic {
            code: format!("CS-{}", w.code),
            severity: DiagnosticSeverity::Warning,
            message: w.message.clone(),
            provenance: Some(Provenance {
                file: w.file.clone(),
                line: w.line.unwrap_or(0),
                column: None,
                source_name: None,
                policy_category: Some("composition".to_string()),
            }),
        });
    }

    // Style warnings
    for w in &styles.warnings {
        builder.add_warning(IrWarning {
            code: format!("SS-{}", style_warning_code(&w.kind)),
            message: w.message.clone(),
            provenance: w.provenance.clone(),
        });
    }
}

fn style_warning_code(kind: &StyleWarningKind) -> &'static str {
    match kind {
        StyleWarningKind::PrecedenceConflict => "PREC",
        StyleWarningKind::UnresolvedClassRef => "UCLS",
        StyleWarningKind::UnresolvedToken => "UTOK",
        StyleWarningKind::InlineOverride => "INOV",
        StyleWarningKind::HardcodedColor => "HCOL",
        StyleWarningKind::AccessibilityConcern => "A11Y",
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration_ir::validate_ir;
    use crate::tsx_parser::{
        ComponentDecl, ComponentKind, FileParse, HookCall, JsxElement, JsxProp,
    };
    use std::collections::BTreeSet;

    fn test_config() -> LoweringConfig {
        LoweringConfig {
            run_id: "test-run-001".to_string(),
            source_project: "test-project".to_string(),
        }
    }

    fn make_project(files: Vec<(&str, FileParse)>) -> ProjectParse {
        ProjectParse {
            files: files.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            symbol_table: BTreeMap::new(),
            component_count: 0,
            hook_usage_count: 0,
            type_count: 0,
            diagnostics: Vec::new(),
            external_imports: BTreeSet::new(),
        }
    }

    fn make_file(path: &str) -> FileParse {
        FileParse {
            file: path.to_string(),
            components: Vec::new(),
            hooks: Vec::new(),
            jsx_elements: Vec::new(),
            types: Vec::new(),
            symbols: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn make_component_file(path: &str) -> FileParse {
        FileParse {
            file: path.to_string(),
            components: vec![ComponentDecl {
                name: "App".to_string(),
                kind: ComponentKind::FunctionComponent,
                is_default_export: true,
                is_named_export: false,
                props_type: None,
                hooks: vec![
                    HookCall {
                        name: "useState".to_string(),
                        binding: Some("count, setCount".to_string()),
                        args_snippet: "0".to_string(),
                        line: 5,
                    },
                    HookCall {
                        name: "useEffect".to_string(),
                        binding: None,
                        args_snippet: "() => { document.title = `Count: ${count}` }, [count]"
                            .to_string(),
                        line: 8,
                    },
                ],
                event_handlers: vec![crate::tsx_parser::EventHandler {
                    event_name: "onClick".to_string(),
                    handler_name: Some("handleClick".to_string()),
                    is_inline: false,
                    line: 12,
                }],
                line: 1,
            }],
            hooks: Vec::new(),
            jsx_elements: vec![
                JsxElement {
                    tag: "div".to_string(),
                    is_component: false,
                    is_fragment: false,
                    is_self_closing: false,
                    props: vec![
                        JsxProp {
                            name: "className".to_string(),
                            is_spread: false,
                            value_snippet: Some("\"container\"".to_string()),
                        },
                        JsxProp {
                            name: "style".to_string(),
                            is_spread: false,
                            value_snippet: Some("{{ display: 'flex', color: '#333' }}".to_string()),
                        },
                    ],
                    line: 15,
                },
                JsxElement {
                    tag: "button".to_string(),
                    is_component: false,
                    is_fragment: false,
                    is_self_closing: false,
                    props: vec![JsxProp {
                        name: "onClick".to_string(),
                        is_spread: false,
                        value_snippet: Some("{handleClick}".to_string()),
                    }],
                    line: 16,
                },
            ],
            types: Vec::new(),
            symbols: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    // ── Basic pipeline ──────────────────────────────────────────────────

    #[test]
    fn empty_project_produces_valid_ir() {
        let project = make_project(vec![]);
        let result = lower_project(&test_config(), &project);
        let errors = validate_ir(&result.ir);
        assert!(errors.is_empty(), "Validation errors: {:?}", errors);
        assert_eq!(result.ir.schema_version, "migration-ir-v1");
    }

    #[test]
    fn single_file_project_lowers() {
        let project = make_project(vec![("src/App.tsx", make_file("src/App.tsx"))]);
        let result = lower_project(&test_config(), &project);
        let errors = validate_ir(&result.ir);
        assert!(errors.is_empty(), "Validation errors: {:?}", errors);
        assert_eq!(result.ir.metadata.source_file_count, 1);
    }

    #[test]
    fn component_file_produces_complete_ir() {
        let project = make_project(vec![("src/App.tsx", make_component_file("src/App.tsx"))]);
        let result = lower_project(&test_config(), &project);
        let errors = validate_ir(&result.ir);
        assert!(errors.is_empty(), "Validation errors: {:?}", errors);

        // Should have view nodes from composition semantics.
        assert!(!result.ir.view_tree.nodes.is_empty(), "Expected view nodes");

        // Should have state variables.
        assert!(
            !result.ir.state_graph.variables.is_empty(),
            "Expected state variables"
        );

        // Should have effects.
        assert!(
            !result.ir.effect_registry.effects.is_empty(),
            "Expected effects"
        );

        // Should have style intent (tokens or layouts).
        assert!(
            !result.ir.style_intent.layouts.is_empty() || !result.ir.style_intent.tokens.is_empty(),
            "Expected style data"
        );
    }

    // ── View tree lowering ──────────────────────────────────────────────

    #[test]
    fn view_tree_preserves_roots() {
        let project = make_project(vec![("src/App.tsx", make_component_file("src/App.tsx"))]);
        let composition = composition_semantics::extract_composition_semantics(&project);
        let styles = style_semantics::extract_style_semantics(&project);
        let state_model =
            state_effects::build_project_state_model(&project.files, &BTreeMap::new());

        let result = lower_to_ir(
            &test_config(),
            &project,
            &composition,
            &styles,
            &state_model,
        );

        // Roots in IR should match composition.
        let comp_tree = composition_semantics::to_view_tree(&composition);
        assert_eq!(result.ir.view_tree.roots.len(), comp_tree.roots.len());
    }

    // ── State graph lowering ────────────────────────────────────────────

    #[test]
    fn state_scope_mapping() {
        assert_eq!(map_state_scope(&StateVarScope::Local), StateScope::Local);
        assert_eq!(map_state_scope(&StateVarScope::Reducer), StateScope::Local);
        assert_eq!(map_state_scope(&StateVarScope::Ref), StateScope::Local);
        assert_eq!(
            map_state_scope(&StateVarScope::Context),
            StateScope::Context
        );
        assert_eq!(
            map_state_scope(&StateVarScope::ExternalStore),
            StateScope::Global
        );
        assert_eq!(map_state_scope(&StateVarScope::Url), StateScope::Route);
        assert_eq!(map_state_scope(&StateVarScope::Server), StateScope::Server);
    }

    // ── Event classification ────────────────────────────────────────────

    #[test]
    fn event_kind_classification() {
        assert_eq!(classify_event_kind("onClick"), EventKind::UserInput);
        assert_eq!(classify_event_kind("onKeyDown"), EventKind::UserInput);
        assert_eq!(classify_event_kind("onChange"), EventKind::UserInput);
        assert_eq!(classify_event_kind("onSubmit"), EventKind::UserInput);
        assert_eq!(classify_event_kind("onMouseEnter"), EventKind::UserInput);
        assert_eq!(classify_event_kind("onScroll"), EventKind::UserInput);
        assert_eq!(
            classify_event_kind("componentDidMount"),
            EventKind::Lifecycle
        );
        assert_eq!(classify_event_kind("timerTick"), EventKind::Timer);
        assert_eq!(classify_event_kind("fetchData"), EventKind::Network);
        assert_eq!(classify_event_kind("customAction"), EventKind::Custom);
    }

    // ── Effect kind mapping ─────────────────────────────────────────────

    #[test]
    fn effect_kind_mapping() {
        assert_eq!(
            map_effect_kind(&EffectClassification::DataFetch),
            EffectKind::Network
        );
        assert_eq!(
            map_effect_kind(&EffectClassification::DomManipulation),
            EffectKind::Dom
        );
        assert_eq!(
            map_effect_kind(&EffectClassification::Timer),
            EffectKind::Timer
        );
        assert_eq!(
            map_effect_kind(&EffectClassification::Sync),
            EffectKind::Storage
        );
        assert_eq!(
            map_effect_kind(&EffectClassification::Telemetry),
            EffectKind::Telemetry
        );
        assert_eq!(
            map_effect_kind(&EffectClassification::Unknown),
            EffectKind::Other
        );
    }

    // ── Determinism ─────────────────────────────────────────────────────

    #[test]
    fn lowering_is_deterministic() {
        let project = make_project(vec![("src/App.tsx", make_component_file("src/App.tsx"))]);

        let result1 = lower_project(&test_config(), &project);
        let result2 = lower_project(&test_config(), &project);

        // Structural determinism: same number of nodes, variables, effects.
        assert_eq!(
            result1.ir.view_tree.nodes.len(),
            result2.ir.view_tree.nodes.len(),
            "View tree node count differs across runs"
        );
        assert_eq!(
            result1.ir.state_graph.variables.len(),
            result2.ir.state_graph.variables.len(),
            "State variable count differs across runs"
        );
        assert_eq!(
            result1.ir.effect_registry.effects.len(),
            result2.ir.effect_registry.effects.len(),
            "Effect count differs across runs"
        );
        // Node IDs must be identical (content-addressable).
        let ids1: BTreeSet<_> = result1.ir.view_tree.nodes.keys().collect();
        let ids2: BTreeSet<_> = result2.ir.view_tree.nodes.keys().collect();
        assert_eq!(ids1, ids2, "View tree node IDs differ across runs");
    }

    // ── Diagnostics ─────────────────────────────────────────────────────

    #[test]
    fn diagnostics_for_empty_view_tree() {
        // Empty file with no components → L002 info diagnostic.
        let project = make_project(vec![("src/empty.tsx", make_file("src/empty.tsx"))]);
        let result = lower_project(&test_config(), &project);

        // No view nodes means no L002 either (only triggers if nodes exist but no roots).
        assert!(
            result.diagnostics.is_empty() || result.diagnostics.iter().all(|d| d.code != "L002")
        );
    }

    // ── Config ──────────────────────────────────────────────────────────

    #[test]
    fn config_propagates_to_ir() {
        let config = LoweringConfig {
            run_id: "custom-run-42".to_string(),
            source_project: "my-react-app".to_string(),
        };
        let project = make_project(vec![]);
        let result = lower_project(&config, &project);
        assert_eq!(result.ir.run_id, "custom-run-42");
        assert_eq!(result.ir.source_project, "my-react-app");
    }

    // ── Serialization ───────────────────────────────────────────────────

    #[test]
    fn ir_serializes_to_json() {
        let project = make_project(vec![("src/App.tsx", make_component_file("src/App.tsx"))]);
        let result = lower_project(&test_config(), &project);
        let json = serde_json::to_string(&result.ir).unwrap();
        assert!(!json.is_empty());

        // Roundtrip.
        let deserialized: MigrationIr = serde_json::from_str(&json).unwrap();
        assert_eq!(result.ir.schema_version, deserialized.schema_version);
        assert_eq!(result.ir.run_id, deserialized.run_id);
    }

    // ── Multi-component ─────────────────────────────────────────────────

    #[test]
    fn multi_component_file() {
        let mut file = make_component_file("src/App.tsx");
        file.components.push(ComponentDecl {
            name: "Header".to_string(),
            kind: ComponentKind::FunctionComponent,
            is_default_export: false,
            is_named_export: true,
            props_type: Some("HeaderProps".to_string()),
            hooks: vec![HookCall {
                name: "useContext".to_string(),
                binding: Some("theme".to_string()),
                args_snippet: "ThemeContext".to_string(),
                line: 20,
            }],
            event_handlers: Vec::new(),
            line: 18,
        });

        let project = make_project(vec![("src/App.tsx", file)]);
        let result = lower_project(&test_config(), &project);
        let errors = validate_ir(&result.ir);
        assert!(errors.is_empty(), "Validation errors: {:?}", errors);

        // Should have at least one state var (from the App component's useState).
        assert!(
            !result.ir.state_graph.variables.is_empty(),
            "Expected state variables from multi-component file"
        );
    }

    // ── Capability forwarding ───────────────────────────────────────────

    #[test]
    fn capabilities_forwarded_from_state_model() {
        let project = make_project(vec![("src/App.tsx", make_component_file("src/App.tsx"))]);
        let composition = composition_semantics::extract_composition_semantics(&project);
        let styles = style_semantics::extract_style_semantics(&project);
        let state_model =
            state_effects::build_project_state_model(&project.files, &BTreeMap::new());

        let result = lower_to_ir(
            &test_config(),
            &project,
            &composition,
            &styles,
            &state_model,
        );

        // Capability sets should match state model.
        assert_eq!(
            result.ir.capabilities.required,
            state_model.required_capabilities
        );
        assert_eq!(
            result.ir.capabilities.optional,
            state_model.optional_capabilities
        );
    }
}
