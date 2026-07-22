use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub id: EvidenceId,
    pub anchor: String,
    pub kind: FindingKind,
    pub path: String,
    pub line: Option<usize>,
    pub metrics: Vec<FindingMetric>,
    pub construct: QualityConstruct,
    pub mechanism: SignalMechanism,
    pub issue_id: Option<IssueKey>,
    pub message: String,
    pub related_locations: Vec<RelatedLocation>,
    pub flow_witness: Option<FlowWitness>,
}

impl Finding {
    pub fn refresh_id(&mut self) {
        if let Some(witness) = &self.flow_witness {
            self.anchor = format!(
                "flow:{}:{}:{}",
                witness.policy, witness.source.id, witness.sink.id
            );
        }
        self.id = stable_finding_id(self);
    }

    pub fn recommendation(&self) -> &'static str {
        recommendation_for_kind(self.kind)
    }
}

pub fn recommendation_for_kind(kind: FindingKind) -> &'static str {
    KIND_RECOMMENDATIONS
        .iter()
        .find_map(|(candidate, recommendation)| (*candidate == kind).then_some(*recommendation))
        .unwrap_or(
            "Review the finding and choose the smallest refactor that reduces the reported risk.",
        )
}

const KIND_RECOMMENDATIONS: &[(FindingKind, &str)] = &[
    (
        FindingKind::LargeFile,
        "Split the file around cohesive responsibilities and move shared helpers behind clear module boundaries.",
    ),
    (
        FindingKind::LargeDirectory,
        "Group related files into focused subdirectories with explicit ownership boundaries.",
    ),
    (
        FindingKind::DebtMarker,
        "Resolve the marked debt or replace the marker with an owner, rationale, and tracking reference.",
    ),
    (
        FindingKind::SimilarFunctions,
        "Extract the shared behavior into a common helper or deliberately separate the variants if they should evolve independently.",
    ),
    (
        FindingKind::LongFunction,
        "Extract named steps until the function has one clear orchestration path.",
    ),
    (
        FindingKind::ComplexFunction,
        "Simplify branching with guard clauses, smaller decision helpers, or a clearer state model.",
    ),
    (
        FindingKind::DeepNesting,
        "Flatten control flow with early returns and extracted helpers for nested branches.",
    ),
    (
        FindingKind::ManyParameters,
        "Introduce a small parameter object or split the function by responsibility.",
    ),
    (
        FindingKind::ReadabilityRisk,
        "Extract named steps or narrower collaborators around the combined size, branching, nesting, or parameter pressure.",
    ),
    (
        FindingKind::LargeType,
        "Separate independent responsibilities into smaller types or move behavior to collaborators.",
    ),
    (
        FindingKind::LargePublicSurface,
        "Reduce public API exposure to the stable operations callers actually need.",
    ),
    (
        FindingKind::ImportHeavyFile,
        "Review dependencies and split orchestration, domain logic, and adapters into narrower modules.",
    ),
    (
        FindingKind::FunctionProliferation,
        "Consolidate tiny related functions into cohesive units or move them near their owning abstraction.",
    ),
    (
        FindingKind::UnusedFunction,
        "Delete the unused function or add the missing call path if it is intentionally exposed.",
    ),
    (
        FindingKind::RepeatedLiteral,
        "Replace repeated literals with a named constant or domain concept where the value has shared meaning.",
    ),
    (
        FindingKind::RepeatedErrorPattern,
        "Centralize repeated error handling in a helper, result mapper, or shared policy.",
    ),
    (
        FindingKind::TestDuplication,
        "Extract common test setup into fixtures while keeping each assertion path explicit.",
    ),
    (
        FindingKind::HappyPathOnlyTests,
        "Add focused failure, boundary, and malformed-input cases around the same behavior.",
    ),
    (
        FindingKind::FileNamingDrift,
        "Normalize file naming within the directory or split mixed conventions by layer.",
    ),
    (
        FindingKind::DirectoryDrift,
        "Reorganize mixed concepts into directories that match domain or layer ownership.",
    ),
    (
        FindingKind::DataClump,
        "Introduce a named value object for fields that repeatedly travel together.",
    ),
    (
        FindingKind::ParallelImplementation,
        "Merge parallel implementations behind one abstraction or document why both variants must remain.",
    ),
    (
        FindingKind::ShadowedAbstraction,
        "Route callers through the existing abstraction instead of maintaining a local duplicate.",
    ),
    (
        FindingKind::DuplicateTypeShape,
        "Consolidate duplicate type shapes or introduce a shared DTO/model with explicit conversion points.",
    ),
    (
        FindingKind::ConfigKeyDrift,
        "Centralize related configuration keys and keep aliases documented at the boundary.",
    ),
    (
        FindingKind::FixtureFactoryDrift,
        "Consolidate fixture factories so test data defaults come from one named source.",
    ),
    (
        FindingKind::GenericBucketDrift,
        "Move generic bucket contents into modules named for the concept they own.",
    ),
    (
        FindingKind::AdapterBoundaryBypass,
        "Route boundary access through the adapter instead of reaching across layers directly.",
    ),
    (
        FindingKind::AdapterFlowBypass,
        "Route the witnessed value transfer through the declared adapter boundary, preserving the existing behavior at that seam.",
    ),
    (
        FindingKind::StaleCompatibilityPath,
        "Remove the compatibility path if callers have migrated or add an explicit sunset plan.",
    ),
    (
        FindingKind::MissingDocumentationSet,
        "Add the missing documentation files or update the documentation index to match supported docs.",
    ),
    (
        FindingKind::MissingUserGuide,
        "Document the user-facing workflow, including commands, options, and expected output.",
    ),
    (
        FindingKind::MissingReportSchemaDocs,
        "Update the report schema reference to include current serialized fields and compatibility notes.",
    ),
    (
        FindingKind::MissingMetricsModelDocs,
        "Document how raw metrics, percentiles, findings, and coverage are computed.",
    ),
    (
        FindingKind::MissingArchitectureDocs,
        "Add architecture notes that explain module boundaries and detector/reporting flow.",
    ),
    (
        FindingKind::StaleCliDocumentation,
        "Update CLI documentation so listed flags and defaults match the parser.",
    ),
    (
        FindingKind::StaleSchemaDocumentation,
        "Update schema documentation for the current report fields and finding kinds.",
    ),
    (
        FindingKind::DependencyCycle,
        "Break the cycle by moving shared contracts to a lower-level module or inverting one dependency.",
    ),
    (
        FindingKind::DependencyHub,
        "Review the hub for mixed responsibilities and split fan-in/fan-out behind narrower interfaces.",
    ),
    (
        FindingKind::UnityAssemblyCycle,
        "Break the asmdef cycle by moving shared contracts into a lower-level runtime assembly.",
    ),
    (
        FindingKind::UnityAssemblyHub,
        "Split the assembly by responsibility or narrow its asmdef references.",
    ),
    (
        FindingKind::UnityUnresolvedAssemblyReference,
        "Correct the asmdef name or GUID reference and restore the referenced package or local assembly.",
    ),
    (
        FindingKind::UnityRuntimeEditorDependency,
        "Move Editor-only code behind an Editor asmdef and remove the runtime-to-Editor edge.",
    ),
    (
        FindingKind::UnityDuplicateGuid,
        "Regenerate one duplicated meta GUID and let Unity rewrite its references.",
    ),
    (
        FindingKind::UnityMissingMeta,
        "Restore the asset's meta file from version control or let Unity generate it, then commit it.",
    ),
    (
        FindingKind::UnityOrphanMeta,
        "Restore the matching asset or remove the orphan meta file.",
    ),
    (
        FindingKind::UnityBrokenAssetReference,
        "Restore or reassign the referenced asset and commit the repaired text serialization.",
    ),
    (
        FindingKind::UnityMissingScript,
        "Restore the MonoScript or remove and replace the missing component in the scene or prefab.",
    ),
    (
        FindingKind::UnityNonTextSerialization,
        "Set Asset Serialization Mode to Force Text so references can be reviewed and merged safely.",
    ),
    (
        FindingKind::UnitySceneBuildDrift,
        "Add the scene to Build Settings if it ships, or move it to a clearly non-shipping location.",
    ),
    (
        FindingKind::UnityLargeScene,
        "Split the scene into additive scenes or streamed content with explicit ownership.",
    ),
    (
        FindingKind::UnityLargePrefab,
        "Decompose the prefab into focused nested prefabs.",
    ),
    (
        FindingKind::UnitySerializedFieldBloat,
        "Group related configuration into serializable value objects or narrower components.",
    ),
    (
        FindingKind::UnityLifecycleOverload,
        "Move lifecycle responsibilities into collaborators and keep the component as orchestration.",
    ),
    (
        FindingKind::UnityExpensiveFrameCall,
        "Cache the resolved component, object, or resource outside the frame-loop path.",
    ),
    (
        FindingKind::UnityEditorApiInRuntime,
        "Move the API use into an Editor assembly or guard it with UNITY_EDITOR.",
    ),
    (
        FindingKind::UnityUnbalancedEventSubscription,
        "Pair subscriptions with deterministic unsubscription in the matching lifecycle path.",
    ),
];

pub fn stable_finding_id(finding: &Finding) -> EvidenceId {
    let input = format!(
        "rf4\0{}\0{}",
        serialized_finding_kind(finding.kind),
        finding.anchor
    );
    EvidenceId(format!("rf4-{:016x}", fnv1a64(input.as_bytes())))
}

pub fn serialized_finding_kind(kind: FindingKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

pub(super) fn normalize_identity_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

pub(super) fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

impl Serialize for Finding {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Finding", 13)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("anchor", &self.anchor)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("line", &self.line)?;
        state.serialize_field("metrics", &self.metrics)?;
        state.serialize_field("construct", &self.construct)?;
        state.serialize_field("mechanism", &self.mechanism)?;
        state.serialize_field("issue_id", &self.issue_id)?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("recommendation", &self.recommendation())?;
        state.serialize_field("related_locations", serialized_related_locations(self))?;
        state.serialize_field("flow_witness", &self.flow_witness)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Finding {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct SerializedFinding {
            id: EvidenceId,
            anchor: String,
            kind: FindingKind,
            path: String,
            line: Option<usize>,
            metrics: Vec<FindingMetric>,
            construct: QualityConstruct,
            mechanism: SignalMechanism,
            issue_id: Option<IssueKey>,
            message: String,
            recommendation: String,
            related_locations: Vec<RelatedLocation>,
            #[serde(default)]
            flow_witness: Option<FlowWitness>,
        }

        let value = SerializedFinding::deserialize(deserializer)?;
        if value.recommendation != recommendation_for_kind(value.kind) {
            return Err(serde::de::Error::custom(
                "finding recommendation does not match its detector kind",
            ));
        }
        Ok(Self {
            id: value.id,
            anchor: value.anchor,
            kind: value.kind,
            path: value.path,
            line: value.line,
            metrics: value.metrics,
            construct: value.construct,
            mechanism: value.mechanism,
            issue_id: value.issue_id,
            message: value.message,
            related_locations: value.related_locations,
            flow_witness: value.flow_witness,
        })
    }
}

fn serialized_related_locations(finding: &Finding) -> &[RelatedLocation] {
    if finding.kind == FindingKind::SimilarFunctions
        && finding.related_locations.len() > SERIALIZED_SIMILAR_LOCATION_LIMIT
    {
        &finding.related_locations[..SERIALIZED_SIMILAR_LOCATION_LIMIT]
    } else {
        &finding.related_locations
    }
}
