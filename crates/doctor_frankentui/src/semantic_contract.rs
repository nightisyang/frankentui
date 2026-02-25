use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const BUILTIN_CONTRACT_JSON: &str =
    include_str!("../contracts/opentui_semantic_equivalence_v1.json");
const SUPPORTED_SCHEMA_VERSION: &str = "sem-eq-contract-v1";
const BUILTIN_TRANSFORMATION_POLICY_JSON: &str =
    include_str!("../contracts/opentui_transformation_policy_v1.json");
const SUPPORTED_TRANSFORMATION_POLICY_SCHEMA_VERSION: &str = "transform-policy-v1";
const BUILTIN_EVIDENCE_MANIFEST_JSON: &str =
    include_str!("../contracts/opentui_evidence_manifest_v1.json");
const SUPPORTED_EVIDENCE_MANIFEST_SCHEMA_VERSION: &str = "evidence-manifest-v1";
const BUILTIN_CONFIDENCE_MODEL_JSON: &str =
    include_str!("../contracts/opentui_confidence_model_v1.json");
const SUPPORTED_CONFIDENCE_MODEL_SCHEMA_VERSION: &str = "confidence-model-v1";
const REQUIRED_POLICY_CATEGORIES: [&str; 6] = [
    "state",
    "layout",
    "style",
    "effects",
    "accessibility",
    "terminal_capability",
];

#[derive(Debug, Error)]
pub enum SemanticContractError {
    #[error("failed to parse semantic contract JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("semantic contract validation failed: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, SemanticContractError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SemanticEquivalenceContract {
    pub contract_id: String,
    pub schema_version: String,
    pub contract_version: String,
    pub equivalence_axes: EquivalenceAxes,
    pub visual_tolerance_policy: VisualTolerancePolicy,
    pub improvement_envelope: ImprovementEnvelope,
    pub deterministic_tie_breakers: Vec<TieBreakerRule>,
    pub clauses: Vec<ContractClause>,
    pub validator_clause_map: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EquivalenceAxes {
    pub state_transition: String,
    pub event_ordering: String,
    pub side_effect_observability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VisualTolerancePolicy {
    pub strict_classes: Vec<String>,
    pub strict_policy: String,
    pub perceptual_classes: Vec<String>,
    pub perceptual_policy: String,
    pub max_perceptual_delta: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImprovementEnvelope {
    pub allowed_dimensions: Vec<String>,
    pub forbidden_rewrites: Vec<String>,
    pub required_safeguards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TieBreakerRule {
    pub priority: u16,
    pub rule_id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContractClause {
    pub clause_id: String,
    pub title: String,
    pub category: String,
    pub requirement: String,
    pub severity: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransformationHandlingClass {
    Exact,
    Approximate,
    ExtendFtui,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TransformationRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyCategoryDefinition {
    pub category_id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyConstructCatalogEntry {
    pub construct_signature: String,
    pub category_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransformationPolicyCell {
    pub construct_signature: String,
    pub handling_class: TransformationHandlingClass,
    pub rationale: String,
    pub risk_level: TransformationRiskLevel,
    pub fallback_behavior: String,
    pub user_messaging: String,
    pub planner_strategy: String,
    pub semantic_clause_links: Vec<String>,
    pub certification_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannerConsumptionProjection {
    pub required_fields: Vec<String>,
    pub deterministic_sort_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CertificationConsumptionProjection {
    pub required_fields: Vec<String>,
    pub required_risk_levels: Vec<TransformationRiskLevel>,
    pub requires_clause_traceability: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransformationPolicyMatrix {
    pub policy_id: String,
    pub schema_version: String,
    pub policy_version: String,
    pub categories: Vec<PolicyCategoryDefinition>,
    pub construct_catalog: Vec<PolicyConstructCatalogEntry>,
    pub policy_cells: Vec<TransformationPolicyCell>,
    pub planner_projection: PlannerConsumptionProjection,
    pub certification_projection: CertificationConsumptionProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannerPolicyRow {
    pub construct_signature: String,
    pub category_id: String,
    pub handling_class: TransformationHandlingClass,
    pub planner_strategy: String,
    pub fallback_behavior: String,
    pub risk_level: TransformationRiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CertificationPolicyRow {
    pub construct_signature: String,
    pub handling_class: TransformationHandlingClass,
    pub risk_level: TransformationRiskLevel,
    pub semantic_clause_links: Vec<String>,
    pub certification_evidence: Vec<String>,
    pub user_messaging: String,
}

impl SemanticEquivalenceContract {
    pub fn parse_and_validate(raw_json: &str) -> Result<Self> {
        let parsed: Self = serde_json::from_str(raw_json)?;
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(SemanticContractError::Validation(format!(
                "unsupported schema_version '{}' (expected '{}')",
                self.schema_version, SUPPORTED_SCHEMA_VERSION
            )));
        }
        if self.contract_id.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "contract_id must not be empty".to_string(),
            ));
        }
        if self.clauses.is_empty() {
            return Err(SemanticContractError::Validation(
                "clauses must not be empty".to_string(),
            ));
        }
        if self.deterministic_tie_breakers.is_empty() {
            return Err(SemanticContractError::Validation(
                "deterministic_tie_breakers must not be empty".to_string(),
            ));
        }
        if self.validator_clause_map.is_empty() {
            return Err(SemanticContractError::Validation(
                "validator_clause_map must not be empty".to_string(),
            ));
        }

        let mut clause_ids = BTreeSet::new();
        for clause in &self.clauses {
            if clause.clause_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "clause_id must not be empty".to_string(),
                ));
            }
            if !clause_ids.insert(clause.clause_id.clone()) {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate clause_id '{}'",
                    clause.clause_id
                )));
            }
        }

        let mut priorities = BTreeSet::new();
        for rule in &self.deterministic_tie_breakers {
            if rule.rule_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "tie-break rule_id must not be empty".to_string(),
                ));
            }
            if !priorities.insert(rule.priority) {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate tie-break priority '{}'",
                    rule.priority
                )));
            }
        }

        for (validator_id, clauses) in &self.validator_clause_map {
            if validator_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "validator id must not be empty".to_string(),
                ));
            }
            if clauses.is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "validator '{}' has no clause mappings",
                    validator_id
                )));
            }
            for clause_id in clauses {
                if !clause_ids.contains(clause_id) {
                    return Err(SemanticContractError::Validation(format!(
                        "validator '{}' references unknown clause '{}'",
                        validator_id, clause_id
                    )));
                }
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn clause(&self, clause_id: &str) -> Option<&ContractClause> {
        self.clauses
            .iter()
            .find(|clause| clause.clause_id == clause_id)
    }

    #[must_use]
    pub fn clauses_for_validator(&self, validator_id: &str) -> Vec<&ContractClause> {
        let Some(ids) = self.validator_clause_map.get(validator_id) else {
            return Vec::new();
        };
        ids.iter().filter_map(|id| self.clause(id)).collect()
    }
}

impl TransformationPolicyMatrix {
    pub fn parse_and_validate(raw_json: &str) -> Result<Self> {
        let parsed: Self = serde_json::from_str(raw_json)?;
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SUPPORTED_TRANSFORMATION_POLICY_SCHEMA_VERSION {
            return Err(SemanticContractError::Validation(format!(
                "unsupported transformation policy schema_version '{}' (expected '{}')",
                self.schema_version, SUPPORTED_TRANSFORMATION_POLICY_SCHEMA_VERSION
            )));
        }
        if self.policy_id.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "policy_id must not be empty".to_string(),
            ));
        }
        if self.categories.is_empty() {
            return Err(SemanticContractError::Validation(
                "categories must not be empty".to_string(),
            ));
        }
        if self.construct_catalog.is_empty() {
            return Err(SemanticContractError::Validation(
                "construct_catalog must not be empty".to_string(),
            ));
        }
        if self.policy_cells.is_empty() {
            return Err(SemanticContractError::Validation(
                "policy_cells must not be empty".to_string(),
            ));
        }
        if self.planner_projection.required_fields.is_empty() {
            return Err(SemanticContractError::Validation(
                "planner_projection.required_fields must not be empty".to_string(),
            ));
        }
        if self
            .planner_projection
            .deterministic_sort_key
            .trim()
            .is_empty()
        {
            return Err(SemanticContractError::Validation(
                "planner_projection.deterministic_sort_key must not be empty".to_string(),
            ));
        }
        if self.certification_projection.required_fields.is_empty() {
            return Err(SemanticContractError::Validation(
                "certification_projection.required_fields must not be empty".to_string(),
            ));
        }
        if self
            .certification_projection
            .required_risk_levels
            .is_empty()
        {
            return Err(SemanticContractError::Validation(
                "certification_projection.required_risk_levels must not be empty".to_string(),
            ));
        }

        let mut category_ids = BTreeSet::new();
        for category in &self.categories {
            if category.category_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "category_id must not be empty".to_string(),
                ));
            }
            if category.description.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "category '{}' has empty description",
                    category.category_id
                )));
            }
            if !category_ids.insert(category.category_id.clone()) {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate category '{}'",
                    category.category_id
                )));
            }
        }
        for required in REQUIRED_POLICY_CATEGORIES {
            if !category_ids.contains(required) {
                return Err(SemanticContractError::Validation(format!(
                    "required policy category '{}' is missing",
                    required
                )));
            }
        }

        let mut catalog_signature_to_category = BTreeMap::new();
        for entry in &self.construct_catalog {
            if entry.construct_signature.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "construct_signature must not be empty".to_string(),
                ));
            }
            if entry.summary.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' has empty summary",
                    entry.construct_signature
                )));
            }
            if !category_ids.contains(entry.category_id.as_str()) {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' references unknown category '{}'",
                    entry.construct_signature, entry.category_id
                )));
            }
            if catalog_signature_to_category
                .insert(entry.construct_signature.clone(), entry.category_id.clone())
                .is_some()
            {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate construct_signature '{}' in construct_catalog",
                    entry.construct_signature
                )));
            }
        }

        let semantic_contract = load_builtin_semantic_contract()?;
        let semantic_clause_ids = semantic_contract
            .clauses
            .iter()
            .map(|clause| clause.clause_id.as_str())
            .collect::<BTreeSet<_>>();

        let mut seen_policy_signatures = BTreeSet::new();
        let mut previous_signature: Option<&str> = None;
        for cell in &self.policy_cells {
            if cell.construct_signature.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "policy cell construct_signature must not be empty".to_string(),
                ));
            }
            if !catalog_signature_to_category.contains_key(cell.construct_signature.as_str()) {
                return Err(SemanticContractError::Validation(format!(
                    "policy cell '{}' not present in construct_catalog",
                    cell.construct_signature
                )));
            }
            if !seen_policy_signatures.insert(cell.construct_signature.clone()) {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate policy cell for construct '{}'",
                    cell.construct_signature
                )));
            }
            if let Some(previous) = previous_signature
                && cell.construct_signature.as_str() < previous
            {
                return Err(SemanticContractError::Validation(
                    "policy_cells must be sorted by construct_signature for deterministic consumption".to_string(),
                ));
            }
            previous_signature = Some(cell.construct_signature.as_str());
            if cell.rationale.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' has empty rationale",
                    cell.construct_signature
                )));
            }
            if cell.fallback_behavior.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' has empty fallback_behavior",
                    cell.construct_signature
                )));
            }
            if cell.user_messaging.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' has empty user_messaging",
                    cell.construct_signature
                )));
            }
            if cell.planner_strategy.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' has empty planner_strategy",
                    cell.construct_signature
                )));
            }
            if cell.semantic_clause_links.is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' must map to at least one semantic clause",
                    cell.construct_signature
                )));
            }
            for clause_id in &cell.semantic_clause_links {
                if !semantic_clause_ids.contains(clause_id.as_str()) {
                    return Err(SemanticContractError::Validation(format!(
                        "construct '{}' references unknown semantic clause '{}'",
                        cell.construct_signature, clause_id
                    )));
                }
            }
            if cell.certification_evidence.is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct '{}' must define certification_evidence",
                    cell.construct_signature
                )));
            }
            for evidence in &cell.certification_evidence {
                if evidence.trim().is_empty() {
                    return Err(SemanticContractError::Validation(format!(
                        "construct '{}' contains empty certification evidence entry",
                        cell.construct_signature
                    )));
                }
            }
        }

        let catalog_signatures = catalog_signature_to_category
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if seen_policy_signatures != catalog_signatures {
            let missing = catalog_signatures
                .difference(&seen_policy_signatures)
                .cloned()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "construct(s) without explicit handling class: {}",
                    missing.join(", ")
                )));
            }
        }

        let risk_levels = self
            .certification_projection
            .required_risk_levels
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let required_levels = [
            TransformationRiskLevel::Low,
            TransformationRiskLevel::Medium,
            TransformationRiskLevel::High,
            TransformationRiskLevel::Critical,
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        if risk_levels != required_levels {
            return Err(SemanticContractError::Validation(
                "certification_projection.required_risk_levels must include low, medium, high, critical".to_string(),
            ));
        }

        Ok(())
    }

    #[must_use]
    pub fn policy_for_construct(
        &self,
        construct_signature: &str,
    ) -> Option<&TransformationPolicyCell> {
        self.policy_cells
            .iter()
            .find(|cell| cell.construct_signature == construct_signature)
    }

    #[must_use]
    pub fn planner_rows(&self) -> Vec<PlannerPolicyRow> {
        let category_by_signature = self
            .construct_catalog
            .iter()
            .map(|entry| (&entry.construct_signature, &entry.category_id))
            .collect::<BTreeMap<_, _>>();
        let mut rows = self
            .policy_cells
            .iter()
            .map(|cell| PlannerPolicyRow {
                construct_signature: cell.construct_signature.clone(),
                category_id: category_by_signature
                    .get(&cell.construct_signature)
                    .map_or_else(String::new, |category| (*category).clone()),
                handling_class: cell.handling_class,
                planner_strategy: cell.planner_strategy.clone(),
                fallback_behavior: cell.fallback_behavior.clone(),
                risk_level: cell.risk_level,
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.construct_signature.cmp(&b.construct_signature));
        rows
    }

    #[must_use]
    pub fn certification_rows(&self) -> Vec<CertificationPolicyRow> {
        let mut rows = self
            .policy_cells
            .iter()
            .map(|cell| CertificationPolicyRow {
                construct_signature: cell.construct_signature.clone(),
                handling_class: cell.handling_class,
                risk_level: cell.risk_level,
                semantic_clause_links: cell.semantic_clause_links.clone(),
                certification_evidence: cell.certification_evidence.clone(),
                user_messaging: cell.user_messaging.clone(),
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.construct_signature.cmp(&b.construct_signature));
        rows
    }
}

// ---------------------------------------------------------------------------
// Evidence Manifest — deterministic artifact manifest for migration runs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvidenceManifest {
    pub manifest_id: String,
    pub schema_version: String,
    pub manifest_version: String,
    pub run_id: String,
    pub source_fingerprint: SourceFingerprint,
    pub stages: Vec<StageRecord>,
    pub generated_code_fingerprint: GeneratedCodeFingerprint,
    pub certification_verdict: CertificationVerdict,
    pub determinism_attestation: DeterminismAttestation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceFingerprint {
    pub repo_url: Option<String>,
    pub repo_commit: Option<String>,
    pub local_path: Option<String>,
    pub source_hash: String,
    pub lockfiles: Vec<LockfileEntry>,
    pub parser_versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LockfileEntry {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StageRecord {
    pub stage_id: String,
    pub stage_index: u32,
    pub correlation_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub status: StageStatus,
    pub input_hash: String,
    pub output_hash: String,
    pub artifact_paths: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Ok,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GeneratedCodeFingerprint {
    pub code_hash: String,
    pub formatter_version: String,
    pub linter_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CertificationVerdict {
    pub verdict: VerdictOutcome,
    pub confidence: f64,
    pub test_pass_count: u32,
    pub test_fail_count: u32,
    pub test_skip_count: u32,
    pub semantic_clause_coverage: SemanticClauseCoverage,
    pub benchmark_summary: BenchmarkSummary,
    pub risk_flags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerdictOutcome {
    Accept,
    Hold,
    Reject,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SemanticClauseCoverage {
    pub covered: Vec<String>,
    pub uncovered: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkSummary {
    pub latency_p50_ms: f64,
    pub latency_p99_ms: f64,
    pub throughput_ops_per_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeterminismAttestation {
    pub identical_runs_count: u32,
    pub manifest_hash_stable: bool,
    pub divergence_detected: bool,
}

// ---------------------------------------------------------------------------
// JSONL evidence record — one structured line per stage event
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageEvidenceRecord {
    pub event: String,
    pub run_id: String,
    pub correlation_id: String,
    pub stage_id: String,
    pub stage_index: u32,
    pub timestamp: String,
    pub status: StageStatus,
    pub input_hash: String,
    pub output_hash: String,
    pub artifact_count: u32,
    pub error: Option<String>,
}

impl EvidenceManifest {
    pub fn parse_and_validate(raw_json: &str) -> Result<Self> {
        let parsed: Self = serde_json::from_str(raw_json)?;
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SUPPORTED_EVIDENCE_MANIFEST_SCHEMA_VERSION {
            return Err(SemanticContractError::Validation(format!(
                "unsupported evidence manifest schema_version '{}' (expected '{}')",
                self.schema_version, SUPPORTED_EVIDENCE_MANIFEST_SCHEMA_VERSION
            )));
        }
        if self.manifest_id.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "manifest_id must not be empty".to_string(),
            ));
        }
        if self.run_id.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "run_id must not be empty".to_string(),
            ));
        }

        // Source fingerprint validation.
        self.validate_source_fingerprint()?;

        // Stage validation.
        self.validate_stages()?;

        // Generated code fingerprint validation.
        self.validate_generated_code_fingerprint()?;

        // Certification verdict validation.
        self.validate_certification_verdict()?;

        // Determinism attestation validation.
        self.validate_determinism_attestation()?;

        Ok(())
    }

    fn validate_source_fingerprint(&self) -> Result<()> {
        let fp = &self.source_fingerprint;
        if fp.source_hash.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "source_fingerprint.source_hash must not be empty".to_string(),
            ));
        }
        if fp.repo_url.is_none() && fp.local_path.is_none() {
            return Err(SemanticContractError::Validation(
                "source_fingerprint must specify repo_url or local_path".to_string(),
            ));
        }
        if fp.parser_versions.is_empty() {
            return Err(SemanticContractError::Validation(
                "source_fingerprint.parser_versions must not be empty".to_string(),
            ));
        }
        for lockfile in &fp.lockfiles {
            if lockfile.path.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "lockfile path must not be empty".to_string(),
                ));
            }
            if lockfile.sha256.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "lockfile '{}' sha256 must not be empty",
                    lockfile.path
                )));
            }
        }
        Ok(())
    }

    fn validate_stages(&self) -> Result<()> {
        if self.stages.is_empty() {
            return Err(SemanticContractError::Validation(
                "stages must not be empty".to_string(),
            ));
        }

        let mut seen_ids = BTreeSet::new();
        let mut prev_index: Option<u32> = None;
        for stage in &self.stages {
            if stage.stage_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "stage_id must not be empty".to_string(),
                ));
            }
            if !seen_ids.insert(stage.stage_id.clone()) {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate stage_id '{}'",
                    stage.stage_id
                )));
            }
            if let Some(prev) = prev_index {
                if stage.stage_index != prev + 1 {
                    return Err(SemanticContractError::Validation(format!(
                        "stage_index for '{}' is {} but expected {} (must be consecutive)",
                        stage.stage_id,
                        stage.stage_index,
                        prev + 1
                    )));
                }
            } else if stage.stage_index != 0 {
                return Err(SemanticContractError::Validation(format!(
                    "first stage '{}' must have stage_index 0",
                    stage.stage_id
                )));
            }
            prev_index = Some(stage.stage_index);
            if stage.correlation_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "stage '{}' correlation_id must not be empty",
                    stage.stage_id
                )));
            }
            if stage.started_at.trim().is_empty() || stage.finished_at.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "stage '{}' must have non-empty started_at and finished_at",
                    stage.stage_id
                )));
            }
            if stage.input_hash.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "stage '{}' input_hash must not be empty",
                    stage.stage_id
                )));
            }
            if stage.status != StageStatus::Failed && stage.output_hash.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "stage '{}' output_hash must not be empty for non-failed stages",
                    stage.stage_id
                )));
            }
            if stage.status == StageStatus::Failed && stage.error.is_none() {
                return Err(SemanticContractError::Validation(format!(
                    "stage '{}' has status 'failed' but no error message",
                    stage.stage_id
                )));
            }
        }

        // Verify hash chain: each stage's input_hash must equal the previous
        // stage's output_hash (except the first stage).
        for window in self.stages.windows(2) {
            let prev_stage = &window[0];
            let curr_stage = &window[1];
            if prev_stage.status == StageStatus::Ok
                && curr_stage.input_hash != prev_stage.output_hash
            {
                return Err(SemanticContractError::Validation(format!(
                    "hash chain broken: stage '{}' output_hash ({}) != stage '{}' input_hash ({})",
                    prev_stage.stage_id,
                    prev_stage.output_hash,
                    curr_stage.stage_id,
                    curr_stage.input_hash
                )));
            }
        }

        Ok(())
    }

    fn validate_generated_code_fingerprint(&self) -> Result<()> {
        let gcf = &self.generated_code_fingerprint;
        if gcf.code_hash.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "generated_code_fingerprint.code_hash must not be empty".to_string(),
            ));
        }
        if gcf.formatter_version.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "generated_code_fingerprint.formatter_version must not be empty".to_string(),
            ));
        }
        if gcf.linter_version.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "generated_code_fingerprint.linter_version must not be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_certification_verdict(&self) -> Result<()> {
        let cv = &self.certification_verdict;
        if !(0.0..=1.0).contains(&cv.confidence) {
            return Err(SemanticContractError::Validation(format!(
                "certification_verdict.confidence must be in [0.0, 1.0], got {}",
                cv.confidence
            )));
        }
        if cv.test_fail_count > 0 && cv.verdict == VerdictOutcome::Accept {
            return Err(SemanticContractError::Validation(
                "certification_verdict cannot be 'accept' with failing tests".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_determinism_attestation(&self) -> Result<()> {
        let da = &self.determinism_attestation;
        if da.identical_runs_count == 0 {
            return Err(SemanticContractError::Validation(
                "determinism_attestation.identical_runs_count must be > 0".to_string(),
            ));
        }
        if da.divergence_detected && da.manifest_hash_stable {
            return Err(SemanticContractError::Validation(
                "determinism_attestation: divergence_detected=true is inconsistent with manifest_hash_stable=true".to_string(),
            ));
        }
        Ok(())
    }

    /// Reconstruct the stage lineage (ordered stage_id chain with hash links)
    /// from the manifest alone, suitable for deterministic replay.
    #[must_use]
    pub fn stage_lineage(&self) -> Vec<StageLineageEntry> {
        self.stages
            .iter()
            .map(|stage| StageLineageEntry {
                stage_id: stage.stage_id.clone(),
                stage_index: stage.stage_index,
                correlation_id: stage.correlation_id.clone(),
                input_hash: stage.input_hash.clone(),
                output_hash: stage.output_hash.clone(),
                status: stage.status,
            })
            .collect()
    }

    /// Convert all stage records to JSONL evidence records for structured logging.
    #[must_use]
    pub fn to_evidence_records(&self) -> Vec<StageEvidenceRecord> {
        self.stages
            .iter()
            .map(|stage| StageEvidenceRecord {
                event: "stage_completed".to_string(),
                run_id: self.run_id.clone(),
                correlation_id: stage.correlation_id.clone(),
                stage_id: stage.stage_id.clone(),
                stage_index: stage.stage_index,
                timestamp: stage.finished_at.clone(),
                status: stage.status,
                input_hash: stage.input_hash.clone(),
                output_hash: stage.output_hash.clone(),
                artifact_count: stage.artifact_paths.len() as u32,
                error: stage.error.clone(),
            })
            .collect()
    }

    /// Serialize evidence records as JSONL (one JSON object per line).
    #[must_use]
    pub fn evidence_jsonl(&self) -> String {
        self.to_evidence_records()
            .iter()
            .filter_map(|record| serde_json::to_string(record).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Look up a stage by its correlation ID.
    #[must_use]
    pub fn stage_by_correlation_id(&self, correlation_id: &str) -> Option<&StageRecord> {
        self.stages
            .iter()
            .find(|stage| stage.correlation_id == correlation_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageLineageEntry {
    pub stage_id: String,
    pub stage_index: u32,
    pub correlation_id: String,
    pub input_hash: String,
    pub output_hash: String,
    pub status: StageStatus,
}

// ---------------------------------------------------------------------------
// Confidence Model — Bayesian decision model for migration verdicts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConfidenceModel {
    pub model_id: String,
    pub schema_version: String,
    pub model_version: String,
    pub decision_space: DecisionSpace,
    pub loss_matrix: LossMatrix,
    pub prior_config: PriorConfig,
    pub likelihood_sources: Vec<LikelihoodSource>,
    pub decision_boundaries: DecisionBoundaries,
    pub calibration: CalibrationConfig,
    pub fallback_triggers: Vec<FallbackTrigger>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionSpace {
    pub actions: Vec<String>,
    pub states: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LossMatrix {
    pub accept_correct: f64,
    pub accept_incorrect: f64,
    pub hold_correct: f64,
    pub hold_incorrect: f64,
    pub reject_correct: f64,
    pub reject_incorrect: f64,
    pub rollback_cost: f64,
    pub conservative_fallback_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PriorConfig {
    pub semantic_pass_rate_alpha: f64,
    pub semantic_pass_rate_beta: f64,
    pub performance_mean_prior: f64,
    pub performance_variance_prior: f64,
    pub unsupported_feature_penalty_per_item: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LikelihoodSource {
    pub source_id: String,
    pub weight: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DecisionBoundaries {
    pub auto_approve_threshold: f64,
    pub human_review_lower: f64,
    pub human_review_upper: f64,
    pub reject_threshold: f64,
    pub hard_reject_threshold: f64,
    pub rollback_trigger: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationConfig {
    pub credible_interval_width: f64,
    pub conformal_coverage_target: f64,
    pub min_calibration_samples: u32,
    pub recalibration_trigger_drift: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FallbackTrigger {
    pub trigger_id: String,
    pub condition: String,
    pub action: String,
}

// ---------------------------------------------------------------------------
// Bayesian posterior computation and decision engine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BayesianPosterior {
    pub alpha: f64,
    pub beta: f64,
    pub mean: f64,
    pub variance: f64,
    pub credible_lower: f64,
    pub credible_upper: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationDecision {
    AutoApprove,
    HumanReview,
    Reject,
    HardReject,
    Rollback,
    ConservativeFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpectedLossResult {
    pub decision: MigrationDecision,
    pub posterior: BayesianPosterior,
    pub expected_loss_accept: f64,
    pub expected_loss_reject: f64,
    pub expected_loss_hold: f64,
    pub rationale: String,
    pub claim_id: Option<String>,
    pub policy_id: Option<String>,
}

impl ConfidenceModel {
    pub fn parse_and_validate(raw_json: &str) -> Result<Self> {
        let parsed: Self = serde_json::from_str(raw_json)?;
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SUPPORTED_CONFIDENCE_MODEL_SCHEMA_VERSION {
            return Err(SemanticContractError::Validation(format!(
                "unsupported confidence model schema_version '{}' (expected '{}')",
                self.schema_version, SUPPORTED_CONFIDENCE_MODEL_SCHEMA_VERSION
            )));
        }
        if self.model_id.trim().is_empty() {
            return Err(SemanticContractError::Validation(
                "model_id must not be empty".to_string(),
            ));
        }

        self.validate_decision_space()?;
        self.validate_loss_matrix()?;
        self.validate_prior_config()?;
        self.validate_likelihood_sources()?;
        self.validate_decision_boundaries()?;
        self.validate_calibration()?;
        self.validate_fallback_triggers()?;

        Ok(())
    }

    fn validate_decision_space(&self) -> Result<()> {
        let ds = &self.decision_space;
        if ds.actions.is_empty() {
            return Err(SemanticContractError::Validation(
                "decision_space.actions must not be empty".to_string(),
            ));
        }
        if ds.states.is_empty() {
            return Err(SemanticContractError::Validation(
                "decision_space.states must not be empty".to_string(),
            ));
        }
        let required_actions = ["accept", "hold", "reject", "rollback"];
        for required in required_actions {
            if !ds.actions.iter().any(|a| a == required) {
                return Err(SemanticContractError::Validation(format!(
                    "decision_space.actions must include '{required}'"
                )));
            }
        }
        Ok(())
    }

    fn validate_loss_matrix(&self) -> Result<()> {
        let lm = &self.loss_matrix;
        if lm.accept_correct < 0.0 {
            return Err(SemanticContractError::Validation(
                "loss_matrix.accept_correct must be >= 0".to_string(),
            ));
        }
        if lm.accept_incorrect <= 0.0 {
            return Err(SemanticContractError::Validation(
                "loss_matrix.accept_incorrect must be > 0 (accepting wrong is costly)".to_string(),
            ));
        }
        if lm.accept_incorrect <= lm.reject_correct {
            return Err(SemanticContractError::Validation(
                "loss_matrix: accept_incorrect must exceed reject_correct (wrong acceptance is worse than correct rejection)".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_prior_config(&self) -> Result<()> {
        let pc = &self.prior_config;
        if pc.semantic_pass_rate_alpha <= 0.0 || pc.semantic_pass_rate_beta <= 0.0 {
            return Err(SemanticContractError::Validation(
                "prior_config: alpha and beta must be > 0 for valid Beta distribution".to_string(),
            ));
        }
        if pc.performance_variance_prior <= 0.0 {
            return Err(SemanticContractError::Validation(
                "prior_config.performance_variance_prior must be > 0".to_string(),
            ));
        }
        if pc.unsupported_feature_penalty_per_item < 0.0 {
            return Err(SemanticContractError::Validation(
                "prior_config.unsupported_feature_penalty_per_item must be >= 0".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_likelihood_sources(&self) -> Result<()> {
        if self.likelihood_sources.is_empty() {
            return Err(SemanticContractError::Validation(
                "likelihood_sources must not be empty".to_string(),
            ));
        }
        let mut total_weight = 0.0_f64;
        let mut seen_ids = BTreeSet::new();
        for source in &self.likelihood_sources {
            if source.source_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "likelihood source_id must not be empty".to_string(),
                ));
            }
            if !seen_ids.insert(source.source_id.clone()) {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate likelihood source_id '{}'",
                    source.source_id
                )));
            }
            if !(0.0..=1.0).contains(&source.weight) {
                return Err(SemanticContractError::Validation(format!(
                    "likelihood source '{}' weight must be in [0.0, 1.0]",
                    source.source_id
                )));
            }
            total_weight += source.weight;
        }
        if (total_weight - 1.0).abs() > 0.01 {
            return Err(SemanticContractError::Validation(format!(
                "likelihood source weights must sum to ~1.0, got {total_weight:.4}"
            )));
        }
        Ok(())
    }

    fn validate_decision_boundaries(&self) -> Result<()> {
        let db = &self.decision_boundaries;
        let boundaries = [
            ("rollback_trigger", db.rollback_trigger),
            ("hard_reject_threshold", db.hard_reject_threshold),
            ("reject_threshold", db.reject_threshold),
            ("human_review_lower", db.human_review_lower),
            ("human_review_upper", db.human_review_upper),
            ("auto_approve_threshold", db.auto_approve_threshold),
        ];
        for (name, value) in &boundaries {
            if !(0.0..=1.0).contains(value) {
                return Err(SemanticContractError::Validation(format!(
                    "decision_boundary '{name}' must be in [0.0, 1.0], got {value}"
                )));
            }
        }
        // Boundaries must be monotonically ordered.
        if db.rollback_trigger > db.hard_reject_threshold {
            return Err(SemanticContractError::Validation(
                "rollback_trigger must be <= hard_reject_threshold".to_string(),
            ));
        }
        if db.hard_reject_threshold > db.reject_threshold {
            return Err(SemanticContractError::Validation(
                "hard_reject_threshold must be <= reject_threshold".to_string(),
            ));
        }
        if db.reject_threshold > db.human_review_lower {
            return Err(SemanticContractError::Validation(
                "reject_threshold must be <= human_review_lower".to_string(),
            ));
        }
        if db.human_review_lower > db.human_review_upper {
            return Err(SemanticContractError::Validation(
                "human_review_lower must be <= human_review_upper".to_string(),
            ));
        }
        if db.human_review_upper > db.auto_approve_threshold {
            return Err(SemanticContractError::Validation(
                "human_review_upper must be <= auto_approve_threshold".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_calibration(&self) -> Result<()> {
        let cal = &self.calibration;
        if !(0.0..=1.0).contains(&cal.credible_interval_width) {
            return Err(SemanticContractError::Validation(
                "calibration.credible_interval_width must be in [0.0, 1.0]".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&cal.conformal_coverage_target) {
            return Err(SemanticContractError::Validation(
                "calibration.conformal_coverage_target must be in [0.0, 1.0]".to_string(),
            ));
        }
        if cal.min_calibration_samples == 0 {
            return Err(SemanticContractError::Validation(
                "calibration.min_calibration_samples must be > 0".to_string(),
            ));
        }
        if cal.recalibration_trigger_drift <= 0.0 {
            return Err(SemanticContractError::Validation(
                "calibration.recalibration_trigger_drift must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_fallback_triggers(&self) -> Result<()> {
        let mut seen_ids = BTreeSet::new();
        for trigger in &self.fallback_triggers {
            if trigger.trigger_id.trim().is_empty() {
                return Err(SemanticContractError::Validation(
                    "fallback trigger_id must not be empty".to_string(),
                ));
            }
            if !seen_ids.insert(trigger.trigger_id.clone()) {
                return Err(SemanticContractError::Validation(format!(
                    "duplicate fallback trigger_id '{}'",
                    trigger.trigger_id
                )));
            }
            if trigger.condition.trim().is_empty() {
                return Err(SemanticContractError::Validation(format!(
                    "fallback trigger '{}' condition must not be empty",
                    trigger.trigger_id
                )));
            }
            let valid_actions = [
                "accept",
                "hold",
                "reject",
                "rollback",
                "conservative_fallback",
            ];
            if !valid_actions.contains(&trigger.action.as_str()) {
                return Err(SemanticContractError::Validation(format!(
                    "fallback trigger '{}' action '{}' not in decision space",
                    trigger.trigger_id, trigger.action
                )));
            }
        }
        Ok(())
    }

    /// Compute Beta-distribution posterior from prior config and observed evidence.
    #[must_use]
    pub fn compute_posterior(&self, successes: u32, failures: u32) -> BayesianPosterior {
        let alpha = self.prior_config.semantic_pass_rate_alpha + f64::from(successes);
        let beta = self.prior_config.semantic_pass_rate_beta + f64::from(failures);
        let mean = alpha / (alpha + beta);
        let variance = (alpha * beta) / ((alpha + beta).powi(2) * (alpha + beta + 1.0));

        // Approximate credible interval using Normal approximation of Beta distribution.
        let half_width = self.calibration.credible_interval_width / 2.0;
        let z = 1.645; // ~90% CI z-score
        let std_dev = variance.sqrt();
        let credible_lower = (mean - z * std_dev * (1.0 + half_width)).max(0.0);
        let credible_upper = (mean + z * std_dev * (1.0 + half_width)).min(1.0);

        BayesianPosterior {
            alpha,
            beta,
            mean,
            variance,
            credible_lower,
            credible_upper,
        }
    }

    /// Make a migration decision based on posterior mean and decision boundaries.
    #[must_use]
    pub fn decide(&self, posterior: &BayesianPosterior) -> MigrationDecision {
        let db = &self.decision_boundaries;
        let p = posterior.mean;

        if p >= db.auto_approve_threshold {
            MigrationDecision::AutoApprove
        } else if p >= db.human_review_lower {
            MigrationDecision::HumanReview
        } else if p >= db.reject_threshold {
            MigrationDecision::Reject
        } else if p >= db.hard_reject_threshold {
            MigrationDecision::HardReject
        } else if p >= db.rollback_trigger {
            MigrationDecision::Rollback
        } else {
            MigrationDecision::ConservativeFallback
        }
    }

    /// Compute expected loss for each action and select the minimum-loss decision.
    #[must_use]
    pub fn expected_loss_decision(
        &self,
        posterior: &BayesianPosterior,
        claim_id: Option<String>,
        policy_id: Option<String>,
    ) -> ExpectedLossResult {
        let p = posterior.mean;
        let lm = &self.loss_matrix;

        // Expected loss for each action.
        let el_accept = p * lm.accept_correct + (1.0 - p) * lm.accept_incorrect;
        let el_reject = p * lm.reject_incorrect + (1.0 - p) * lm.reject_correct;
        let el_hold = p * lm.hold_correct + (1.0 - p) * lm.hold_incorrect;

        // Select minimum expected loss action, with boundary override.
        let boundary_decision = self.decide(posterior);
        let el_decision = if el_accept <= el_reject && el_accept <= el_hold {
            MigrationDecision::AutoApprove
        } else if el_hold <= el_reject {
            MigrationDecision::HumanReview
        } else {
            MigrationDecision::Reject
        };

        // Use boundary decision as override when it is more conservative.
        let decision = match (boundary_decision, el_decision) {
            (MigrationDecision::ConservativeFallback, _)
            | (MigrationDecision::Rollback, _)
            | (MigrationDecision::HardReject, _) => boundary_decision,
            (MigrationDecision::Reject, MigrationDecision::AutoApprove) => boundary_decision,
            _ => el_decision,
        };

        let rationale = format!(
            "posterior_mean={:.4}, ci=[{:.4},{:.4}], EL(accept)={:.2}, EL(reject)={:.2}, EL(hold)={:.2}, boundary={}",
            p,
            posterior.credible_lower,
            posterior.credible_upper,
            el_accept,
            el_reject,
            el_hold,
            match boundary_decision {
                MigrationDecision::AutoApprove => "auto_approve",
                MigrationDecision::HumanReview => "human_review",
                MigrationDecision::Reject => "reject",
                MigrationDecision::HardReject => "hard_reject",
                MigrationDecision::Rollback => "rollback",
                MigrationDecision::ConservativeFallback => "conservative_fallback",
            }
        );

        ExpectedLossResult {
            decision,
            posterior: posterior.clone(),
            expected_loss_accept: el_accept,
            expected_loss_reject: el_reject,
            expected_loss_hold: el_hold,
            rationale,
            claim_id,
            policy_id,
        }
    }
}

pub fn load_builtin_semantic_contract() -> Result<SemanticEquivalenceContract> {
    SemanticEquivalenceContract::parse_and_validate(BUILTIN_CONTRACT_JSON)
}

pub fn load_builtin_transformation_policy_matrix() -> Result<TransformationPolicyMatrix> {
    TransformationPolicyMatrix::parse_and_validate(BUILTIN_TRANSFORMATION_POLICY_JSON)
}

pub fn load_builtin_evidence_manifest() -> Result<EvidenceManifest> {
    EvidenceManifest::parse_and_validate(BUILTIN_EVIDENCE_MANIFEST_JSON)
}

pub fn load_builtin_confidence_model() -> Result<ConfidenceModel> {
    ConfidenceModel::parse_and_validate(BUILTIN_CONFIDENCE_MODEL_JSON)
}

#[cfg(test)]
mod tests {
    use super::{
        REQUIRED_POLICY_CATEGORIES, Result, SUPPORTED_SCHEMA_VERSION,
        SUPPORTED_TRANSFORMATION_POLICY_SCHEMA_VERSION, SemanticContractError,
        SemanticEquivalenceContract, TransformationPolicyMatrix, load_builtin_semantic_contract,
        load_builtin_transformation_policy_matrix,
    };

    fn load() -> Result<SemanticEquivalenceContract> {
        load_builtin_semantic_contract()
    }

    fn load_policy() -> Result<TransformationPolicyMatrix> {
        load_builtin_transformation_policy_matrix()
    }

    #[test]
    fn builtin_contract_parses_and_validates() {
        let contract = load().expect("builtin contract should parse");
        assert_eq!(contract.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert!(!contract.contract_id.is_empty());
    }

    #[test]
    fn validators_map_to_known_clauses() {
        let contract = load().expect("builtin contract should parse");
        for (validator_id, clause_ids) in &contract.validator_clause_map {
            assert!(
                !clause_ids.is_empty(),
                "validator {} should not have empty clause map",
                validator_id
            );
            for clause_id in clause_ids {
                assert!(
                    contract.clause(clause_id).is_some(),
                    "clause {} should exist",
                    clause_id
                );
            }
        }
    }

    #[test]
    fn tie_break_priorities_are_unique_and_sorted() {
        let contract = load().expect("builtin contract should parse");
        let mut priorities = contract
            .deterministic_tie_breakers
            .iter()
            .map(|rule| rule.priority)
            .collect::<Vec<_>>();
        let mut sorted = priorities.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            priorities.len(),
            sorted.len(),
            "tie-break priorities must be unique"
        );
        priorities.sort_unstable();
        assert_eq!(priorities, sorted);
    }

    #[test]
    fn unknown_validator_has_no_mapped_clauses() {
        let contract = load().expect("builtin contract should parse");
        assert!(
            contract
                .clauses_for_validator("validator-that-does-not-exist")
                .is_empty()
        );
    }

    #[test]
    fn invalid_contract_rejects_unknown_clauses() {
        let invalid = r#"
        {
          "contract_id": "bad",
          "schema_version": "sem-eq-contract-v1",
          "contract_version": "2026-02-25",
          "equivalence_axes": {
            "state_transition": "x",
            "event_ordering": "x",
            "side_effect_observability": "x"
          },
          "visual_tolerance_policy": {
            "strict_classes": ["a"],
            "strict_policy": "x",
            "perceptual_classes": ["b"],
            "perceptual_policy": "x",
            "max_perceptual_delta": 0.1
          },
          "improvement_envelope": {
            "allowed_dimensions": ["x"],
            "forbidden_rewrites": ["x"],
            "required_safeguards": ["x"]
          },
          "deterministic_tie_breakers": [
            {"priority": 1, "rule_id": "tb", "description": "x"}
          ],
          "clauses": [
            {
              "clause_id": "CL-1",
              "title": "t",
              "category": "c",
              "requirement": "r",
              "severity": "high"
            }
          ],
          "validator_clause_map": {
            "v": ["DOES-NOT-EXIST"]
          }
        }
        "#;

        let error =
            SemanticEquivalenceContract::parse_and_validate(invalid).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref message) if message.contains("unknown clause")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn builtin_transformation_policy_matrix_parses_and_validates() {
        let matrix = load_policy().expect("builtin policy matrix should parse");
        assert_eq!(
            matrix.schema_version,
            SUPPORTED_TRANSFORMATION_POLICY_SCHEMA_VERSION
        );
        assert!(!matrix.policy_id.is_empty());
    }

    #[test]
    fn transformation_policy_covers_required_categories() {
        let matrix = load_policy().expect("builtin policy matrix should parse");
        let categories = matrix
            .categories
            .iter()
            .map(|category| category.category_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for required in REQUIRED_POLICY_CATEGORIES {
            assert!(
                categories.contains(required),
                "required category {} should exist",
                required
            );
        }
    }

    #[test]
    fn transformation_policy_explicitly_classifies_every_construct() {
        let matrix = load_policy().expect("builtin policy matrix should parse");
        let catalog_signatures = matrix
            .construct_catalog
            .iter()
            .map(|entry| entry.construct_signature.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let policy_signatures = matrix
            .policy_cells
            .iter()
            .map(|cell| cell.construct_signature.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            catalog_signatures, policy_signatures,
            "every construct must have exactly one policy class"
        );
    }

    #[test]
    fn planner_and_certification_rows_are_deterministic_and_complete() {
        let matrix = load_policy().expect("builtin policy matrix should parse");
        let planner_rows = matrix.planner_rows();
        let certification_rows = matrix.certification_rows();
        assert_eq!(planner_rows.len(), matrix.construct_catalog.len());
        assert_eq!(certification_rows.len(), matrix.construct_catalog.len());
        assert!(
            planner_rows
                .windows(2)
                .all(|window| window[0].construct_signature <= window[1].construct_signature)
        );
        assert!(
            certification_rows
                .iter()
                .all(|row| !row.semantic_clause_links.is_empty()
                    && !row.certification_evidence.is_empty())
        );
    }

    #[test]
    fn invalid_transformation_policy_rejects_missing_construct_classification() {
        let mut matrix = load_policy().expect("builtin policy matrix should parse");
        matrix
            .policy_cells
            .pop()
            .expect("policy cells should not be empty");
        let raw = serde_json::to_string(&matrix).expect("policy matrix should serialize");
        let error = TransformationPolicyMatrix::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref message) if message.contains("without explicit handling class")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_transformation_policy_rejects_unknown_semantic_clause() {
        let mut matrix = load_policy().expect("builtin policy matrix should parse");
        matrix.policy_cells[0].semantic_clause_links = vec!["DOES-NOT-EXIST".to_string()];
        let raw = serde_json::to_string(&matrix).expect("policy matrix should serialize");
        let error = TransformationPolicyMatrix::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref message) if message.contains("unknown semantic clause")),
            "unexpected error: {error}"
        );
    }

    // -----------------------------------------------------------------------
    // Evidence Manifest tests
    // -----------------------------------------------------------------------

    fn load_manifest() -> Result<super::EvidenceManifest> {
        super::load_builtin_evidence_manifest()
    }

    #[test]
    fn builtin_evidence_manifest_parses_and_validates() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        assert_eq!(
            manifest.schema_version,
            super::SUPPORTED_EVIDENCE_MANIFEST_SCHEMA_VERSION
        );
        assert!(!manifest.manifest_id.is_empty());
        assert!(!manifest.run_id.is_empty());
    }

    #[test]
    fn evidence_manifest_stages_are_consecutively_indexed() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        for (i, stage) in manifest.stages.iter().enumerate() {
            assert_eq!(
                stage.stage_index, i as u32,
                "stage '{}' should have index {}",
                stage.stage_id, i
            );
        }
    }

    #[test]
    fn evidence_manifest_hash_chain_is_valid() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        for window in manifest.stages.windows(2) {
            let prev = &window[0];
            let curr = &window[1];
            assert_eq!(
                prev.output_hash, curr.input_hash,
                "hash chain: stage '{}' output should equal stage '{}' input",
                prev.stage_id, curr.stage_id
            );
        }
    }

    #[test]
    fn evidence_manifest_correlation_ids_are_unique() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        let mut seen = std::collections::BTreeSet::new();
        for stage in &manifest.stages {
            assert!(
                seen.insert(&stage.correlation_id),
                "duplicate correlation_id '{}'",
                stage.correlation_id
            );
        }
    }

    #[test]
    fn evidence_manifest_source_fingerprint_has_hash() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        assert!(
            !manifest.source_fingerprint.source_hash.is_empty(),
            "source_hash must not be empty"
        );
        assert!(
            manifest.source_fingerprint.repo_url.is_some()
                || manifest.source_fingerprint.local_path.is_some(),
            "must have repo_url or local_path"
        );
    }

    #[test]
    fn evidence_manifest_certification_verdict_is_consistent() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        let cv = &manifest.certification_verdict;
        assert!(
            (0.0..=1.0).contains(&cv.confidence),
            "confidence must be in [0.0, 1.0]"
        );
        if cv.verdict == super::VerdictOutcome::Accept {
            assert_eq!(
                cv.test_fail_count, 0,
                "accept verdict must have zero failures"
            );
        }
    }

    #[test]
    fn evidence_manifest_determinism_attestation_is_consistent() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        let da = &manifest.determinism_attestation;
        assert!(
            da.identical_runs_count > 0,
            "identical_runs_count must be > 0"
        );
        if da.divergence_detected {
            assert!(
                !da.manifest_hash_stable,
                "divergence detected but manifest_hash_stable is true"
            );
        }
    }

    #[test]
    fn evidence_manifest_stage_lineage_reconstructs_full_chain() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        let lineage = manifest.stage_lineage();
        assert_eq!(lineage.len(), manifest.stages.len());
        for (entry, stage) in lineage.iter().zip(manifest.stages.iter()) {
            assert_eq!(entry.stage_id, stage.stage_id);
            assert_eq!(entry.correlation_id, stage.correlation_id);
            assert_eq!(entry.input_hash, stage.input_hash);
            assert_eq!(entry.output_hash, stage.output_hash);
        }
    }

    #[test]
    fn evidence_manifest_to_jsonl_produces_valid_json_lines() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        let jsonl = manifest.evidence_jsonl();
        let lines = jsonl.lines().collect::<Vec<_>>();
        assert_eq!(
            lines.len(),
            manifest.stages.len(),
            "one JSONL line per stage"
        );
        for line in &lines {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("each JSONL line must be valid JSON");
            assert_eq!(
                parsed["event"], "stage_completed",
                "event field must be stage_completed"
            );
            assert!(
                !parsed["correlation_id"].as_str().unwrap_or("").is_empty(),
                "correlation_id must not be empty"
            );
            assert!(
                !parsed["run_id"].as_str().unwrap_or("").is_empty(),
                "run_id must not be empty"
            );
        }
    }

    #[test]
    fn evidence_manifest_stage_lookup_by_correlation_id() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        for stage in &manifest.stages {
            let found = manifest
                .stage_by_correlation_id(&stage.correlation_id)
                .expect("should find stage by correlation_id");
            assert_eq!(found.stage_id, stage.stage_id);
        }
        assert!(
            manifest.stage_by_correlation_id("nonexistent-id").is_none(),
            "nonexistent correlation_id should return None"
        );
    }

    #[test]
    fn evidence_manifest_round_trip_serialization_is_stable() {
        let manifest = load_manifest().expect("builtin evidence manifest should parse");
        let serialized =
            serde_json::to_string_pretty(&manifest).expect("evidence manifest should serialize");
        let deserialized: super::EvidenceManifest =
            serde_json::from_str(&serialized).expect("should deserialize back");
        assert_eq!(manifest, deserialized, "round-trip must be stable");
    }

    #[test]
    fn invalid_evidence_manifest_rejects_empty_run_id() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        manifest.run_id = String::new();
        let raw = serde_json::to_string(&manifest).expect("should serialize");
        let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("run_id")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_evidence_manifest_rejects_broken_hash_chain() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        if manifest.stages.len() >= 2 {
            manifest.stages[1].input_hash = "sha256:BROKEN".to_string();
            let raw = serde_json::to_string(&manifest).expect("should serialize");
            let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
            assert!(
                matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("hash chain broken")),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn invalid_evidence_manifest_rejects_non_consecutive_indices() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        if manifest.stages.len() >= 2 {
            manifest.stages[1].stage_index = 5;
            let raw = serde_json::to_string(&manifest).expect("should serialize");
            let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
            assert!(
                matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("consecutive")),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn invalid_evidence_manifest_rejects_accept_with_failures() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        manifest.certification_verdict.verdict = super::VerdictOutcome::Accept;
        manifest.certification_verdict.test_fail_count = 3;
        let raw = serde_json::to_string(&manifest).expect("should serialize");
        let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("failing tests")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_evidence_manifest_rejects_divergence_with_stable_hash() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        manifest.determinism_attestation.divergence_detected = true;
        manifest.determinism_attestation.manifest_hash_stable = true;
        let raw = serde_json::to_string(&manifest).expect("should serialize");
        let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("divergence")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_evidence_manifest_rejects_confidence_out_of_range() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        manifest.certification_verdict.confidence = 1.5;
        manifest.certification_verdict.verdict = super::VerdictOutcome::Hold;
        let raw = serde_json::to_string(&manifest).expect("should serialize");
        let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("confidence")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_evidence_manifest_rejects_empty_source_hash() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        manifest.source_fingerprint.source_hash = String::new();
        let raw = serde_json::to_string(&manifest).expect("should serialize");
        let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("source_hash")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_evidence_manifest_rejects_failed_stage_without_error() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        if !manifest.stages.is_empty() {
            let last_idx = manifest.stages.len() - 1;
            manifest.stages[last_idx].status = super::StageStatus::Failed;
            manifest.stages[last_idx].error = None;
            let raw = serde_json::to_string(&manifest).expect("should serialize");
            let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
            assert!(
                matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("failed")),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn invalid_evidence_manifest_rejects_zero_identical_runs() {
        let mut manifest = load_manifest().expect("builtin evidence manifest should parse");
        manifest.determinism_attestation.identical_runs_count = 0;
        let raw = serde_json::to_string(&manifest).expect("should serialize");
        let error = super::EvidenceManifest::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("identical_runs_count")),
            "unexpected error: {error}"
        );
    }

    // -----------------------------------------------------------------------
    // Confidence Model tests
    // -----------------------------------------------------------------------

    fn load_confidence() -> Result<super::ConfidenceModel> {
        super::load_builtin_confidence_model()
    }

    #[test]
    fn builtin_confidence_model_parses_and_validates() {
        let model = load_confidence().expect("builtin confidence model should parse");
        assert_eq!(
            model.schema_version,
            super::SUPPORTED_CONFIDENCE_MODEL_SCHEMA_VERSION
        );
        assert!(!model.model_id.is_empty());
    }

    #[test]
    fn confidence_model_likelihood_weights_sum_to_one() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let total: f64 = model.likelihood_sources.iter().map(|s| s.weight).sum();
        assert!(
            (total - 1.0).abs() < 0.01,
            "likelihood weights must sum to ~1.0, got {total}"
        );
    }

    #[test]
    fn confidence_model_decision_boundaries_are_monotonic() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let db = &model.decision_boundaries;
        assert!(db.rollback_trigger <= db.hard_reject_threshold);
        assert!(db.hard_reject_threshold <= db.reject_threshold);
        assert!(db.reject_threshold <= db.human_review_lower);
        assert!(db.human_review_lower <= db.human_review_upper);
        assert!(db.human_review_upper <= db.auto_approve_threshold);
    }

    #[test]
    fn confidence_model_decision_space_has_required_actions() {
        let model = load_confidence().expect("builtin confidence model should parse");
        for required in ["accept", "hold", "reject", "rollback"] {
            assert!(
                model.decision_space.actions.iter().any(|a| a == required),
                "decision_space.actions must include '{required}'"
            );
        }
    }

    #[test]
    fn confidence_model_loss_matrix_is_consistent() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let lm = &model.loss_matrix;
        assert!(
            lm.accept_incorrect > lm.reject_correct,
            "wrong acceptance must be costlier than correct rejection"
        );
        assert!(lm.accept_correct >= 0.0, "correct acceptance cost >= 0");
    }

    #[test]
    fn confidence_model_fallback_triggers_have_valid_actions() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let valid = [
            "accept",
            "hold",
            "reject",
            "rollback",
            "conservative_fallback",
        ];
        for trigger in &model.fallback_triggers {
            assert!(
                valid.contains(&trigger.action.as_str()),
                "trigger '{}' has invalid action '{}'",
                trigger.trigger_id,
                trigger.action
            );
        }
    }

    #[test]
    fn posterior_computation_with_no_evidence_returns_prior() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let posterior = model.compute_posterior(0, 0);
        let expected_mean = model.prior_config.semantic_pass_rate_alpha
            / (model.prior_config.semantic_pass_rate_alpha
                + model.prior_config.semantic_pass_rate_beta);
        assert!(
            (posterior.mean - expected_mean).abs() < 1e-10,
            "posterior mean with no evidence should equal prior mean"
        );
        assert_eq!(posterior.alpha, model.prior_config.semantic_pass_rate_alpha);
        assert_eq!(posterior.beta, model.prior_config.semantic_pass_rate_beta);
    }

    #[test]
    fn posterior_mean_increases_with_successes() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let p0 = model.compute_posterior(0, 0);
        let p10 = model.compute_posterior(10, 0);
        let p100 = model.compute_posterior(100, 0);
        assert!(p10.mean > p0.mean, "more successes should increase mean");
        assert!(p100.mean > p10.mean, "more successes should increase mean");
    }

    #[test]
    fn posterior_mean_decreases_with_failures() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let p0 = model.compute_posterior(0, 0);
        let p_fail = model.compute_posterior(0, 10);
        assert!(p_fail.mean < p0.mean, "more failures should decrease mean");
    }

    #[test]
    fn posterior_variance_decreases_with_more_data() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let p_small = model.compute_posterior(5, 1);
        let p_large = model.compute_posterior(50, 10);
        assert!(
            p_large.variance < p_small.variance,
            "more data should decrease variance"
        );
    }

    #[test]
    fn posterior_credible_interval_is_within_bounds() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let posterior = model.compute_posterior(20, 3);
        assert!(
            posterior.credible_lower >= 0.0,
            "credible lower must be >= 0"
        );
        assert!(
            posterior.credible_upper <= 1.0,
            "credible upper must be <= 1"
        );
        assert!(
            posterior.credible_lower <= posterior.mean,
            "lower bound must be <= mean"
        );
        assert!(
            posterior.credible_upper >= posterior.mean,
            "upper bound must be >= mean"
        );
    }

    #[test]
    fn decision_high_confidence_auto_approves() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let posterior = model.compute_posterior(200, 1);
        let decision = model.decide(&posterior);
        assert_eq!(
            decision,
            super::MigrationDecision::AutoApprove,
            "very high success rate should auto-approve"
        );
    }

    #[test]
    fn decision_low_confidence_rejects() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let posterior = model.compute_posterior(1, 20);
        let decision = model.decide(&posterior);
        assert!(
            matches!(
                decision,
                super::MigrationDecision::Reject
                    | super::MigrationDecision::HardReject
                    | super::MigrationDecision::Rollback
                    | super::MigrationDecision::ConservativeFallback
            ),
            "low confidence should reject, got {:?}",
            decision
        );
    }

    #[test]
    fn expected_loss_decision_includes_rationale() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let posterior = model.compute_posterior(30, 5);
        let result =
            model.expected_loss_decision(&posterior, Some("claim-1".into()), Some("pol-1".into()));
        assert!(!result.rationale.is_empty(), "rationale must not be empty");
        assert!(
            result.rationale.contains("posterior_mean"),
            "rationale must include posterior_mean"
        );
        assert!(
            result.rationale.contains("EL(accept)"),
            "rationale must include expected loss"
        );
        assert_eq!(result.claim_id.as_deref(), Some("claim-1"));
        assert_eq!(result.policy_id.as_deref(), Some("pol-1"));
    }

    #[test]
    fn expected_loss_decision_round_trip_serialization() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let posterior = model.compute_posterior(15, 3);
        let result = model.expected_loss_decision(&posterior, None, None);
        let serialized = serde_json::to_string_pretty(&result).expect("result should serialize");
        let deserialized: super::ExpectedLossResult =
            serde_json::from_str(&serialized).expect("should deserialize back");
        assert_eq!(result.decision, deserialized.decision);
        assert_eq!(result.rationale, deserialized.rationale);
    }

    #[test]
    fn invalid_confidence_model_rejects_empty_model_id() {
        let mut model = load_confidence().expect("builtin confidence model should parse");
        model.model_id = String::new();
        let raw = serde_json::to_string(&model).expect("should serialize");
        let error = super::ConfidenceModel::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("model_id")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_confidence_model_rejects_unbalanced_weights() {
        let mut model = load_confidence().expect("builtin confidence model should parse");
        for source in &mut model.likelihood_sources {
            source.weight = 0.5;
        }
        let raw = serde_json::to_string(&model).expect("should serialize");
        let error = super::ConfidenceModel::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("sum to")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_confidence_model_rejects_inverted_boundaries() {
        let mut model = load_confidence().expect("builtin confidence model should parse");
        model.decision_boundaries.auto_approve_threshold = 0.1;
        model.decision_boundaries.rollback_trigger = 0.9;
        let raw = serde_json::to_string(&model).expect("should serialize");
        let error = super::ConfidenceModel::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(_)),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn invalid_confidence_model_rejects_negative_alpha() {
        let mut model = load_confidence().expect("builtin confidence model should parse");
        model.prior_config.semantic_pass_rate_alpha = -1.0;
        let raw = serde_json::to_string(&model).expect("should serialize");
        let error = super::ConfidenceModel::parse_and_validate(&raw).expect_err("must fail");
        assert!(
            matches!(error, SemanticContractError::Validation(ref msg) if msg.contains("alpha")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn confidence_model_round_trip_serialization_is_stable() {
        let model = load_confidence().expect("builtin confidence model should parse");
        let serialized =
            serde_json::to_string_pretty(&model).expect("confidence model should serialize");
        let deserialized: super::ConfidenceModel =
            serde_json::from_str(&serialized).expect("should deserialize back");
        assert_eq!(model, deserialized, "round-trip must be stable");
    }
}
