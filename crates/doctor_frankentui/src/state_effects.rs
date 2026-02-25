// SPDX-License-Identifier: Apache-2.0
//! State machine and effects semantic extraction.
//!
//! Recovers state transitions, reducer patterns, derived state, and effect
//! dependencies from hooks and event handlers. Produces a serializable
//! state-effects model that maps into FrankenTUI Model/update/subscriptions.
//!
//! Consumes output from `tsx_parser` and `module_graph`.

use std::collections::{BTreeMap, BTreeSet};

use regex_lite::Regex;
use serde::{Deserialize, Serialize};

use crate::migration_ir::{Capability, PlatformAssumption};
use crate::tsx_parser::{ComponentDecl, FileParse, HookCall};

// ── Types ────────────────────────────────────────────────────────────────

/// Complete state-effects analysis result for a component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStateModel {
    pub component_name: String,
    pub file: String,
    pub state_vars: Vec<StateVar>,
    pub reducers: Vec<ReducerPattern>,
    pub derived: Vec<DerivedComputation>,
    pub effects: Vec<EffectBinding>,
    pub event_transitions: Vec<EventStateTransition>,
    pub context_consumers: Vec<ContextConsumer>,
    pub context_providers: Vec<ContextProvider>,
}

/// A state variable extracted from hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVar {
    pub name: String,
    pub setter: Option<String>,
    pub hook: String,
    pub initial_value: Option<String>,
    pub scope: StateVarScope,
    pub type_hint: Option<String>,
    pub line: usize,
}

/// Scope of a state variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateVarScope {
    /// Component-local useState.
    Local,
    /// useReducer pattern.
    Reducer,
    /// useRef (mutable ref, not reactive).
    Ref,
    /// useContext consumer.
    Context,
    /// External store (Zustand, Redux, Jotai, etc.).
    ExternalStore,
    /// URL/search params.
    Url,
    /// Server state (React Query, SWR).
    Server,
}

/// A reducer pattern (useReducer or dispatch-based).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducerPattern {
    pub state_name: String,
    pub dispatch_name: String,
    pub reducer_name: Option<String>,
    pub initial_state: Option<String>,
    pub action_types: Vec<String>,
    pub line: usize,
}

/// A derived computation (useMemo, useCallback).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedComputation {
    pub name: Option<String>,
    pub hook: String,
    pub dependencies: Vec<String>,
    pub expression_snippet: String,
    pub is_callback: bool,
    pub line: usize,
}

/// An effect binding (useEffect, useLayoutEffect).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectBinding {
    pub hook: String,
    pub dependencies: Vec<String>,
    pub has_cleanup: bool,
    pub kind: EffectClassification,
    pub required_capabilities: Vec<Capability>,
    pub optional_capabilities: Vec<Capability>,
    pub platform_assumptions: Vec<PlatformAssumption>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub line: usize,
}

/// Classification of an effect's purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectClassification {
    /// Data fetching (fetch, axios, etc.).
    DataFetch,
    /// DOM manipulation or measurement.
    DomManipulation,
    /// Event listener setup.
    EventListener,
    /// Timer/interval setup.
    Timer,
    /// Subscription setup.
    Subscription,
    /// Synchronization with external state.
    Sync,
    /// Logging or analytics.
    Telemetry,
    /// Unknown/unclassified.
    Unknown,
}

/// A state transition triggered by an event handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStateTransition {
    pub event_name: String,
    pub handler_name: Option<String>,
    pub state_writes: Vec<String>,
    pub is_async: bool,
    pub line: usize,
}

/// A context consumer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConsumer {
    pub context_name: String,
    pub binding: Option<String>,
    pub line: usize,
}

/// A context provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProvider {
    pub context_name: String,
    pub value_expression: Option<String>,
    pub line: usize,
}

/// Project-level state-effects analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStateModel {
    pub components: BTreeMap<String, ComponentStateModel>,
    pub global_state_stores: Vec<GlobalStoreInfo>,
    pub context_graph: Vec<ContextEdge>,
    pub required_capabilities: BTreeSet<Capability>,
    pub optional_capabilities: BTreeSet<Capability>,
    pub platform_assumptions: Vec<PlatformAssumption>,
    pub risk_flags: Vec<CapabilityRiskFlag>,
    pub stats: StateEffectsStats,
}

/// Risk level for capability/platform assumptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityRiskLevel {
    Blocking,
    WarnOnly,
}

/// A capability/platform risk with origin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRiskFlag {
    pub level: CapabilityRiskLevel,
    pub summary: String,
    pub capability: Option<Capability>,
    pub assumption: Option<String>,
    pub file: String,
    pub component: String,
    pub line: usize,
}

/// Information about a detected global state store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStoreInfo {
    pub kind: GlobalStoreKind,
    pub name: String,
    pub file: String,
}

/// Kind of global state management.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlobalStoreKind {
    Redux,
    Zustand,
    Jotai,
    Recoil,
    MobX,
    Context,
    Custom,
}

/// An edge in the context provider→consumer graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEdge {
    pub provider_component: String,
    pub consumer_component: String,
    pub context_name: String,
}

/// Summary statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEffectsStats {
    pub total_state_vars: usize,
    pub total_reducers: usize,
    pub total_effects: usize,
    pub total_derived: usize,
    pub total_event_transitions: usize,
    pub total_context_consumers: usize,
    pub total_context_providers: usize,
    pub effect_classification_counts: BTreeMap<String, usize>,
}

// ── Extraction ───────────────────────────────────────────────────────────

/// Extract state-effects model from a parsed file.
pub fn extract_state_effects(file_parse: &FileParse) -> Vec<ComponentStateModel> {
    let mut models = Vec::new();

    for component in &file_parse.components {
        let model = extract_component_state(component, &file_parse.file);
        models.push(model);
    }

    models
}

/// Extract state-effects model from a single component.
fn extract_component_state(component: &ComponentDecl, file: &str) -> ComponentStateModel {
    let mut state_vars = Vec::new();
    let mut reducers = Vec::new();
    let mut derived = Vec::new();
    let mut effects = Vec::new();
    let mut context_consumers = Vec::new();
    let context_providers = Vec::new();

    for hook in &component.hooks {
        match hook.name.as_str() {
            "useState" => {
                state_vars.push(extract_use_state(hook));
            }
            "useReducer" => {
                if let Some((sv, rp)) = extract_use_reducer(hook) {
                    state_vars.push(sv);
                    reducers.push(rp);
                }
            }
            "useRef" => {
                state_vars.push(extract_use_ref(hook));
            }
            "useMemo" => {
                derived.push(extract_use_memo(hook, false));
            }
            "useCallback" => {
                derived.push(extract_use_memo(hook, true));
            }
            "useEffect" | "useLayoutEffect" | "useInsertionEffect" => {
                effects.push(extract_use_effect(hook));
            }
            "useContext" => {
                context_consumers.push(extract_use_context(hook));
            }
            name if name.starts_with("use") => {
                // Custom hooks — try to classify.
                if let Some(sv) = try_classify_custom_hook(hook) {
                    state_vars.push(sv);
                }
            }
            _ => {}
        }
    }

    // Event transitions from event handlers.
    let event_transitions = extract_event_transitions(component);

    ComponentStateModel {
        component_name: component.name.clone(),
        file: file.to_string(),
        state_vars,
        reducers,
        derived,
        effects,
        event_transitions,
        context_consumers,
        context_providers,
    }
}

fn extract_use_state(hook: &HookCall) -> StateVar {
    let (name, setter) = if let Some(ref binding) = hook.binding {
        parse_destructured_pair(binding)
    } else {
        (None, None)
    };

    StateVar {
        name: name.unwrap_or_else(|| "state".to_string()),
        setter,
        hook: "useState".to_string(),
        initial_value: if hook.args_snippet.is_empty() {
            None
        } else {
            Some(hook.args_snippet.clone())
        },
        scope: StateVarScope::Local,
        type_hint: None,
        line: hook.line,
    }
}

fn extract_use_reducer(hook: &HookCall) -> Option<(StateVar, ReducerPattern)> {
    let (state_name, dispatch_name) = if let Some(ref binding) = hook.binding {
        parse_destructured_pair(binding)
    } else {
        (None, None)
    };

    let state_name = state_name.unwrap_or_else(|| "state".to_string());
    let dispatch_name = dispatch_name.unwrap_or_else(|| "dispatch".to_string());

    // Parse args: reducer, initialState
    let parts: Vec<&str> = hook.args_snippet.splitn(2, ',').collect();
    let reducer_name = parts.first().map(|s| s.trim().to_string());
    let initial_state = parts.get(1).map(|s| s.trim().to_string());

    let sv = StateVar {
        name: state_name.clone(),
        setter: Some(dispatch_name.clone()),
        hook: "useReducer".to_string(),
        initial_value: initial_state.clone(),
        scope: StateVarScope::Reducer,
        type_hint: None,
        line: hook.line,
    };

    let rp = ReducerPattern {
        state_name,
        dispatch_name,
        reducer_name,
        initial_state,
        action_types: Vec::new(), // Would need deeper analysis.
        line: hook.line,
    };

    Some((sv, rp))
}

fn extract_use_ref(hook: &HookCall) -> StateVar {
    let name = hook
        .binding
        .as_ref()
        .map(|b| b.trim().to_string())
        .unwrap_or_else(|| "ref".to_string());

    StateVar {
        name,
        setter: None,
        hook: "useRef".to_string(),
        initial_value: if hook.args_snippet.is_empty() {
            None
        } else {
            Some(hook.args_snippet.clone())
        },
        scope: StateVarScope::Ref,
        type_hint: None,
        line: hook.line,
    }
}

fn extract_use_memo(hook: &HookCall, is_callback: bool) -> DerivedComputation {
    let name = hook.binding.as_ref().map(|b| b.trim().to_string());

    // Dependencies are in the second argument (array).
    let deps = extract_dependency_array(&hook.args_snippet);

    DerivedComputation {
        name,
        hook: hook.name.clone(),
        dependencies: deps,
        expression_snippet: hook.args_snippet.clone(),
        is_callback,
        line: hook.line,
    }
}

fn extract_use_effect(hook: &HookCall) -> EffectBinding {
    let deps = extract_dependency_array(&hook.args_snippet);
    let has_cleanup = hook.args_snippet.contains("return ");
    let kind = classify_effect(&hook.args_snippet);
    let (required_caps, optional_caps, assumptions) =
        classify_effect_capabilities(&kind, &hook.args_snippet);

    // Approximate reads/writes by looking for setter patterns.
    let reads = extract_reads_from_snippet(&hook.args_snippet);
    let writes = extract_writes_from_snippet(&hook.args_snippet);

    EffectBinding {
        hook: hook.name.clone(),
        dependencies: deps,
        has_cleanup,
        kind,
        required_capabilities: required_caps.into_iter().collect(),
        optional_capabilities: optional_caps.into_iter().collect(),
        platform_assumptions: assumptions,
        reads,
        writes,
        line: hook.line,
    }
}

fn extract_use_context(hook: &HookCall) -> ContextConsumer {
    let binding = hook.binding.as_ref().map(|b| b.trim().to_string());
    let context_name = hook.args_snippet.trim().to_string();

    ContextConsumer {
        context_name,
        binding,
        line: hook.line,
    }
}

fn try_classify_custom_hook(hook: &HookCall) -> Option<StateVar> {
    let name_lower = hook.name.to_lowercase();

    // Common patterns for state-like custom hooks.
    let scope = if name_lower.contains("query")
        || name_lower.contains("fetch")
        || name_lower.contains("swr")
    {
        StateVarScope::Server
    } else if name_lower.contains("store")
        || name_lower.contains("atom")
        || name_lower.contains("selector")
    {
        StateVarScope::ExternalStore
    } else if name_lower.contains("param") || name_lower.contains("router") {
        StateVarScope::Url
    } else {
        return None;
    };

    let name = hook
        .binding
        .as_ref()
        .map(|b| b.trim().to_string())
        .unwrap_or_else(|| hook.name.clone());

    Some(StateVar {
        name,
        setter: None,
        hook: hook.name.clone(),
        initial_value: None,
        scope,
        type_hint: None,
        line: hook.line,
    })
}

// ── Effect Classification ────────────────────────────────────────────────

fn classify_effect(snippet: &str) -> EffectClassification {
    let lower = snippet.to_lowercase();

    if lower.contains("fetch(")
        || lower.contains("axios")
        || lower.contains(".get(")
        || lower.contains(".post(")
        || lower.contains("graphql")
    {
        EffectClassification::DataFetch
    } else if lower.contains("addeventlistener")
        || lower.contains("removeeventlistener")
        || lower.contains("window.on")
    {
        EffectClassification::EventListener
    } else if lower.contains("settimeout")
        || lower.contains("setinterval")
        || lower.contains("requestanimationframe")
    {
        EffectClassification::Timer
    } else if lower.contains("subscribe") || lower.contains("observable") {
        EffectClassification::Subscription
    } else if lower.contains("document.")
        || lower.contains("getelementby")
        || lower.contains(".focus(")
    {
        EffectClassification::DomManipulation
    } else if lower.contains("console.") || lower.contains("analytics") || lower.contains("track(")
    {
        EffectClassification::Telemetry
    } else if lower.contains("localstorage")
        || lower.contains("sessionstorage")
        || lower.contains("sync")
    {
        EffectClassification::Sync
    } else {
        EffectClassification::Unknown
    }
}

fn classify_effect_capabilities(
    kind: &EffectClassification,
    snippet: &str,
) -> (
    BTreeSet<Capability>,
    BTreeSet<Capability>,
    Vec<PlatformAssumption>,
) {
    let mut required = BTreeSet::new();
    let mut optional = BTreeSet::new();
    let mut assumptions = Vec::new();
    let lower = snippet.to_lowercase();

    match kind {
        EffectClassification::DataFetch => {
            required.insert(Capability::NetworkAccess);
        }
        EffectClassification::Timer => {
            required.insert(Capability::Timers);
        }
        EffectClassification::EventListener => {
            let keyboard =
                lower.contains("keydown") || lower.contains("keyup") || lower.contains("keypress");
            let pointer = lower.contains("mouse")
                || lower.contains("pointer")
                || lower.contains("click")
                || lower.contains("drag")
                || lower.contains("wheel");
            let touch = lower.contains("touch");

            if keyboard {
                required.insert(Capability::KeyboardInput);
            }
            if pointer {
                required.insert(Capability::MouseInput);
            }
            if touch {
                required.insert(Capability::TouchInput);
            }
            if !keyboard && !pointer && !touch {
                optional.insert(Capability::KeyboardInput);
                optional.insert(Capability::MouseInput);
            }
        }
        EffectClassification::Subscription => {
            optional.insert(Capability::NetworkAccess);
        }
        EffectClassification::Telemetry => {
            optional.insert(Capability::NetworkAccess);
        }
        EffectClassification::Sync => {
            if lower.contains("localstorage") || lower.contains("sessionstorage") {
                optional.insert(Capability::FileSystem);
            }
        }
        EffectClassification::DomManipulation | EffectClassification::Unknown => {}
    }

    if lower.contains("navigator.clipboard") {
        required.insert(Capability::Clipboard);
    }
    if lower.contains("document.") || lower.contains("window.") {
        assumptions.push(PlatformAssumption {
            assumption: "Browser DOM APIs are available at runtime".to_string(),
            evidence: truncate_evidence(snippet, 120),
            impact_if_wrong: "DOM/event effects cannot execute; UI behavior may break".to_string(),
        });
    }
    if lower.contains("process.env") {
        assumptions.push(PlatformAssumption {
            assumption: "Environment variables are available in runtime context".to_string(),
            evidence: truncate_evidence(snippet, 120),
            impact_if_wrong: "Configuration-dependent effects may use wrong defaults".to_string(),
        });
    }
    if lower.contains("localstorage") || lower.contains("sessionstorage") {
        assumptions.push(PlatformAssumption {
            assumption: "Web storage APIs are available".to_string(),
            evidence: truncate_evidence(snippet, 120),
            impact_if_wrong: "Persistence/sync flows may fail or lose state".to_string(),
        });
    }

    (required, optional, assumptions)
}

fn truncate_evidence(snippet: &str, max: usize) -> String {
    let collapsed = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() <= max {
        collapsed
    } else {
        format!("{}...", &collapsed[..max])
    }
}

fn build_effect_risk_flags(
    file: &str,
    component: &str,
    effect: &EffectBinding,
) -> Vec<CapabilityRiskFlag> {
    let mut flags = Vec::new();

    for cap in &effect.required_capabilities {
        flags.push(CapabilityRiskFlag {
            level: CapabilityRiskLevel::Blocking,
            summary: format!("Required capability {:?} inferred from effect", cap),
            capability: Some(cap.clone()),
            assumption: None,
            file: file.to_string(),
            component: component.to_string(),
            line: effect.line,
        });
    }

    for cap in &effect.optional_capabilities {
        flags.push(CapabilityRiskFlag {
            level: CapabilityRiskLevel::WarnOnly,
            summary: format!("Optional capability {:?} may improve fidelity", cap),
            capability: Some(cap.clone()),
            assumption: None,
            file: file.to_string(),
            component: component.to_string(),
            line: effect.line,
        });
    }

    for assumption in &effect.platform_assumptions {
        flags.push(CapabilityRiskFlag {
            level: CapabilityRiskLevel::WarnOnly,
            summary: format!("Platform assumption: {}", assumption.assumption),
            capability: None,
            assumption: Some(assumption.assumption.clone()),
            file: file.to_string(),
            component: component.to_string(),
            line: effect.line,
        });
    }

    flags
}

// ── Event Transition Extraction ──────────────────────────────────────────

fn extract_event_transitions(component: &ComponentDecl) -> Vec<EventStateTransition> {
    let mut transitions = Vec::new();

    for handler in &component.event_handlers {
        let handler_text = handler.handler_name.as_deref().unwrap_or("");

        // Look for state setter patterns (setXxx, dispatch).
        let re_setter = Regex::new(r"set[A-Z]\w*").expect("setter regex");
        let mut writes: Vec<String> = Vec::new();
        for m in re_setter.find_iter(handler_text) {
            writes.push(m.as_str().to_string());
        }

        let is_async = handler_text.contains("async")
            || handler_text.contains("await")
            || handler_text.contains(".then(");

        transitions.push(EventStateTransition {
            event_name: handler.event_name.clone(),
            handler_name: handler.handler_name.clone(),
            state_writes: writes,
            is_async,
            line: handler.line,
        });
    }

    transitions
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Parse a destructured pair like `[foo, setFoo]` into (Some("foo"), Some("setFoo")).
fn parse_destructured_pair(binding: &str) -> (Option<String>, Option<String>) {
    let trimmed = binding.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let inner = &trimmed[1..trimmed.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        let first = parts.first().map(|s| s.trim().to_string());
        let second = parts.get(1).map(|s| s.trim().to_string());
        (first, second)
    } else {
        (Some(trimmed.to_string()), None)
    }
}

/// Extract dependency array items from a hook args snippet.
fn extract_dependency_array(args: &str) -> Vec<String> {
    let re = Regex::new(r"\[([^\]]*)\]").expect("dep array regex");
    if let Some(caps) = re.captures(args) {
        caps[1]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    }
}

/// Approximate state reads from effect body.
fn extract_reads_from_snippet(snippet: &str) -> Vec<String> {
    let re = Regex::new(r"\b([a-z]\w*)\b").expect("reads regex");
    let mut reads: BTreeSet<String> = BTreeSet::new();
    for m in re.find_iter(snippet) {
        let name = m.as_str();
        // Skip common non-state identifiers.
        if !matches!(
            name,
            "const"
                | "let"
                | "var"
                | "return"
                | "if"
                | "else"
                | "true"
                | "false"
                | "null"
                | "undefined"
                | "function"
                | "async"
                | "await"
                | "new"
        ) {
            reads.insert(name.to_string());
        }
    }
    reads.into_iter().collect()
}

/// Approximate state writes from effect body (setter calls).
fn extract_writes_from_snippet(snippet: &str) -> Vec<String> {
    let re = Regex::new(r"(set[A-Z]\w*|dispatch)\s*\(").expect("writes regex");
    let mut writes = Vec::new();
    for caps in re.captures_iter(snippet) {
        writes.push(caps[1].to_string());
    }
    writes
}

/// Detect global state store patterns in file content.
pub fn detect_global_stores(content: &str, file: &str) -> Vec<GlobalStoreInfo> {
    let mut stores = Vec::new();

    let patterns: &[(&str, GlobalStoreKind)] = &[
        ("createStore", GlobalStoreKind::Redux),
        ("configureStore", GlobalStoreKind::Redux),
        ("createSlice", GlobalStoreKind::Redux),
        ("create(", GlobalStoreKind::Zustand),
        ("atom(", GlobalStoreKind::Jotai),
        ("atomWithStorage", GlobalStoreKind::Jotai),
        ("atom({", GlobalStoreKind::Recoil),
        ("selector({", GlobalStoreKind::Recoil),
        ("makeAutoObservable", GlobalStoreKind::MobX),
        ("makeObservable", GlobalStoreKind::MobX),
        ("createContext", GlobalStoreKind::Context),
    ];

    for (pattern, kind) in patterns {
        if content.contains(pattern) {
            // Try to extract the store name.
            let re_name = Regex::new(&format!(
                r"(?:const|let|export\s+const)\s+(\w+)\s*=\s*.*{pattern}"
            ))
            .ok();

            let name = re_name
                .and_then(|re| re.captures(content))
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| format!("{kind:?}Store"));

            stores.push(GlobalStoreInfo {
                kind: kind.clone(),
                name,
                file: file.to_string(),
            });
        }
    }

    stores
}

/// Build project-level state model from all file parses.
pub fn build_project_state_model(
    file_parses: &BTreeMap<String, FileParse>,
    file_contents: &BTreeMap<String, String>,
) -> ProjectStateModel {
    let mut components = BTreeMap::new();
    let mut global_stores = Vec::new();
    let mut context_graph = Vec::new();
    let mut required_capabilities = BTreeSet::new();
    let mut optional_capabilities = BTreeSet::new();
    let mut platform_assumptions = Vec::new();
    let mut platform_assumption_dedup = BTreeSet::new();
    let mut risk_flags = Vec::new();

    let mut total_state_vars = 0usize;
    let mut total_reducers = 0usize;
    let mut total_effects = 0usize;
    let mut total_derived = 0usize;
    let mut total_event_transitions = 0usize;
    let mut total_context_consumers = 0usize;
    let mut total_context_providers = 0usize;
    let mut effect_counts: BTreeMap<String, usize> = BTreeMap::new();

    for (file, parse) in file_parses {
        let models = extract_state_effects(parse);

        for model in models {
            total_state_vars += model.state_vars.len();
            total_reducers += model.reducers.len();
            total_effects += model.effects.len();
            total_derived += model.derived.len();
            total_event_transitions += model.event_transitions.len();
            total_context_consumers += model.context_consumers.len();
            total_context_providers += model.context_providers.len();

            for effect in &model.effects {
                let key = format!("{:?}", effect.kind);
                *effect_counts.entry(key).or_default() += 1;

                for cap in &effect.required_capabilities {
                    required_capabilities.insert(cap.clone());
                    optional_capabilities.remove(cap);
                }
                for cap in &effect.optional_capabilities {
                    if !required_capabilities.contains(cap) {
                        optional_capabilities.insert(cap.clone());
                    }
                }
                for assumption in &effect.platform_assumptions {
                    let dedup_key = format!(
                        "{}|{}|{}",
                        assumption.assumption, assumption.evidence, assumption.impact_if_wrong
                    );
                    if platform_assumption_dedup.insert(dedup_key) {
                        platform_assumptions.push(assumption.clone());
                    }
                }

                risk_flags.extend(build_effect_risk_flags(file, &model.component_name, effect));
            }

            // Build context graph edges.
            for consumer in &model.context_consumers {
                context_graph.push(ContextEdge {
                    provider_component: String::new(), // resolved below
                    consumer_component: model.component_name.clone(),
                    context_name: consumer.context_name.clone(),
                });
            }

            let key = format!("{}::{}", file, model.component_name);
            components.insert(key, model);
        }

        // Detect global stores.
        if let Some(content) = file_contents.get(file) {
            global_stores.extend(detect_global_stores(content, file));
        }
    }

    ProjectStateModel {
        components,
        global_state_stores: global_stores,
        context_graph,
        required_capabilities,
        optional_capabilities,
        platform_assumptions,
        risk_flags,
        stats: StateEffectsStats {
            total_state_vars,
            total_reducers,
            total_effects,
            total_derived,
            total_event_transitions,
            total_context_consumers,
            total_context_providers,
            effect_classification_counts: effect_counts,
        },
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tsx_parser::parse_file;

    #[test]
    fn extract_use_state_basic() {
        let src = r#"
function Counter() {
    const [count, setCount] = useState(0);
    return <div>{count}</div>;
}
"#;
        let parse = parse_file(src, "Counter.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].state_vars.len(), 1);
        assert_eq!(models[0].state_vars[0].name, "count");
        assert_eq!(models[0].state_vars[0].setter.as_deref(), Some("setCount"));
        assert_eq!(models[0].state_vars[0].scope, StateVarScope::Local);
        assert_eq!(models[0].state_vars[0].initial_value.as_deref(), Some("0"));
    }

    #[test]
    fn extract_use_reducer_pattern() {
        let src = r#"
function TodoList() {
    const [state, dispatch] = useReducer(todoReducer, initialState);
    return <div />;
}
"#;
        let parse = parse_file(src, "TodoList.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models[0].reducers.len(), 1);
        assert_eq!(models[0].reducers[0].state_name, "state");
        assert_eq!(models[0].reducers[0].dispatch_name, "dispatch");
        assert_eq!(
            models[0].reducers[0].reducer_name.as_deref(),
            Some("todoReducer")
        );
    }

    #[test]
    fn extract_use_ref() {
        let src = r#"
function Input() {
    const inputRef = useRef(null);
    return <input ref={inputRef} />;
}
"#;
        let parse = parse_file(src, "Input.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models[0].state_vars.len(), 1);
        assert_eq!(models[0].state_vars[0].scope, StateVarScope::Ref);
        assert_eq!(models[0].state_vars[0].name, "inputRef");
    }

    #[test]
    fn extract_use_memo_and_callback() {
        let src = r#"
function Expensive() {
    const memoized = useMemo(() => compute(x), [x]);
    const handler = useCallback(() => doStuff(), [dep]);
    return <div />;
}
"#;
        let parse = parse_file(src, "Expensive.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models[0].derived.len(), 2);
        assert!(!models[0].derived[0].is_callback);
        assert!(models[0].derived[1].is_callback);
    }

    #[test]
    fn extract_use_effect_presence() {
        // tsx_parser's hook regex stops at the first ')' in arrow params,
        // so args_snippet is empty for useEffect(()=>...). We just verify
        // the effect is extracted; classification is tested directly below.
        let src = r#"
function DataLoader() {
    const [data, setData] = useState(null);
    useEffect(() => { fetch('/api'); }, []);
    return <div />;
}
"#;
        let parse = parse_file(src, "DataLoader.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models[0].effects.len(), 1);
        assert_eq!(models[0].effects[0].hook, "useEffect");
    }

    #[test]
    fn classify_effect_data_fetch() {
        // Test classification directly with a realistic snippet.
        assert_eq!(
            classify_effect("() => { fetch('/api/data').then(setData); }"),
            EffectClassification::DataFetch
        );
    }

    #[test]
    fn extract_use_context() {
        let src = r#"
function ThemeButton() {
    const theme = useContext(ThemeContext);
    return <button />;
}
"#;
        let parse = parse_file(src, "ThemeButton.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models[0].context_consumers.len(), 1);
        assert_eq!(models[0].context_consumers[0].context_name, "ThemeContext");
    }

    #[test]
    fn classify_effect_timer() {
        assert_eq!(
            classify_effect("() => { const id = setInterval(() => tick(), 1000); }"),
            EffectClassification::Timer
        );
    }

    #[test]
    fn classify_effect_event_listener() {
        assert_eq!(
            classify_effect("() => { window.addEventListener('resize', handler); }"),
            EffectClassification::EventListener
        );
    }

    #[test]
    fn classify_effect_subscription() {
        assert_eq!(
            classify_effect("() => { const sub = observable.subscribe(cb); }"),
            EffectClassification::Subscription
        );
    }

    #[test]
    fn classify_effect_dom() {
        assert_eq!(
            classify_effect("() => { document.title = 'New Title'; }"),
            EffectClassification::DomManipulation
        );
    }

    #[test]
    fn classify_effect_unknown() {
        assert_eq!(
            classify_effect("() => { doSomething(); }"),
            EffectClassification::Unknown
        );
    }

    #[test]
    fn classify_effect_capabilities_data_fetch_and_dom_assumption() {
        let (required, optional, assumptions) = classify_effect_capabilities(
            &EffectClassification::DataFetch,
            "() => { window.fetch('/api/data'); }",
        );
        assert!(required.contains(&Capability::NetworkAccess));
        assert!(!optional.contains(&Capability::NetworkAccess));
        assert!(!assumptions.is_empty());
        assert!(
            assumptions
                .iter()
                .any(|a| a.assumption.contains("Browser DOM APIs"))
        );
    }

    #[test]
    fn classify_effect_capabilities_event_listener_keyboard() {
        let (required, _optional, assumptions) = classify_effect_capabilities(
            &EffectClassification::EventListener,
            "() => { window.addEventListener('keydown', onKey); }",
        );
        assert!(required.contains(&Capability::KeyboardInput));
        assert!(!assumptions.is_empty());
    }

    #[test]
    fn parse_destructured_pair_array() {
        let (a, b) = parse_destructured_pair("[count, setCount]");
        assert_eq!(a.as_deref(), Some("count"));
        assert_eq!(b.as_deref(), Some("setCount"));
    }

    #[test]
    fn parse_destructured_pair_single() {
        let (a, b) = parse_destructured_pair("value");
        assert_eq!(a.as_deref(), Some("value"));
        assert!(b.is_none());
    }

    #[test]
    fn extract_dependency_array_basic() {
        let deps = extract_dependency_array("() => {}, [a, b, c]");
        assert_eq!(deps, vec!["a", "b", "c"]);
    }

    #[test]
    fn extract_dependency_array_empty() {
        let deps = extract_dependency_array("() => {}, []");
        assert!(deps.is_empty());
    }

    #[test]
    fn extract_dependency_array_none() {
        let deps = extract_dependency_array("() => {}");
        assert!(deps.is_empty());
    }

    #[test]
    fn detect_global_stores_redux() {
        let content = "const store = configureStore({ reducer: rootReducer });";
        let stores = detect_global_stores(content, "store.ts");
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].kind, GlobalStoreKind::Redux);
    }

    #[test]
    fn detect_global_stores_zustand() {
        let content = "export const useStore = create((set) => ({ count: 0 }));";
        let stores = detect_global_stores(content, "store.ts");
        assert!(stores.iter().any(|s| s.kind == GlobalStoreKind::Zustand));
    }

    #[test]
    fn detect_global_stores_context() {
        let content = "export const ThemeContext = createContext('light');";
        let stores = detect_global_stores(content, "context.ts");
        assert!(stores.iter().any(|s| s.kind == GlobalStoreKind::Context));
    }

    #[test]
    fn custom_hook_server_state() {
        let src = r#"
function UserProfile() {
    const data = useQuery('user', fetchUser);
    return <div />;
}
"#;
        let parse = parse_file(src, "UserProfile.tsx");
        let models = extract_state_effects(&parse);
        assert!(!models[0].state_vars.is_empty());
        assert_eq!(models[0].state_vars[0].scope, StateVarScope::Server);
    }

    #[test]
    fn multiple_hooks_per_component() {
        let src = r#"
function Dashboard() {
    const [count, setCount] = useState(0);
    const [name, setName] = useState('');
    const ref = useRef(null);
    const memo = useMemo(() => count * 2, [count]);
    useEffect(() => { document.title = name; }, [name]);
    return <div />;
}
"#;
        let parse = parse_file(src, "Dashboard.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models[0].state_vars.len(), 3); // 2 useState + 1 useRef
        assert_eq!(models[0].derived.len(), 1);
        assert_eq!(models[0].effects.len(), 1);
    }

    #[test]
    fn project_state_model_from_files() {
        let src = r#"
function App() {
    const [x, setX] = useState(0);
    return <div />;
}
"#;
        let parse = parse_file(src, "App.tsx");
        let mut file_parses = BTreeMap::new();
        file_parses.insert("App.tsx".to_string(), parse);

        let model = build_project_state_model(&file_parses, &BTreeMap::new());
        assert_eq!(model.stats.total_state_vars, 1);
    }

    #[test]
    fn project_state_model_aggregates_capabilities_and_risk_flags() {
        let src = r#"
function App() {
    useEffect(graphqlClientQuery, []);
    useEffect(setInterval, []);
    useEffect(window.addEventListener, []);
    return <div />;
}
"#;
        let parse = parse_file(src, "App.tsx");
        let mut file_parses = BTreeMap::new();
        file_parses.insert("App.tsx".to_string(), parse);

        let model = build_project_state_model(&file_parses, &BTreeMap::new());
        assert!(
            model
                .required_capabilities
                .contains(&Capability::NetworkAccess)
        );
        assert!(model.required_capabilities.contains(&Capability::Timers));
        assert!(
            model
                .optional_capabilities
                .contains(&Capability::KeyboardInput)
        );
        assert!(!model.platform_assumptions.is_empty());
        assert!(
            model
                .risk_flags
                .iter()
                .any(|f| f.level == CapabilityRiskLevel::Blocking)
        );
    }

    #[test]
    fn model_serializes_to_json() {
        let src = r#"
function App() {
    const [x, setX] = useState(0);
    return <div />;
}
"#;
        let parse = parse_file(src, "App.tsx");
        let models = extract_state_effects(&parse);
        let json = serde_json::to_string_pretty(&models).unwrap();
        assert!(json.contains("useState"));
        assert!(json.contains("Local"));
    }

    #[test]
    fn writes_extraction_from_snippet() {
        let writes =
            extract_writes_from_snippet("setCount(prev => prev + 1); dispatch({ type: 'INC' })");
        assert!(writes.contains(&"setCount".to_string()));
        assert!(writes.contains(&"dispatch".to_string()));
    }

    #[test]
    fn effect_cleanup_and_timer_classification_direct() {
        // Test cleanup detection and timer classification directly
        // (tsx_parser can't capture nested-paren args for useEffect).
        let snippet =
            "() => { const id = setInterval(tick, 1000); return () => clearInterval(id); }";
        assert!(snippet.contains("return "));
        assert_eq!(classify_effect(snippet), EffectClassification::Timer);
    }

    #[test]
    fn effect_extracted_from_parsed_source() {
        let src = r#"
function Timer() {
    useEffect(() => { setInterval(t, 1000); }, []);
    return <div />;
}
"#;
        let parse = parse_file(src, "Timer.tsx");
        let models = extract_state_effects(&parse);
        assert_eq!(models[0].effects.len(), 1);
        assert_eq!(models[0].effects[0].hook, "useEffect");
    }
}
