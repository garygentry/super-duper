use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSession {
    pub id: i64,
    pub name: String,
    pub roots_json: String,
    pub ignore_patterns_json: String,
    pub cloud_policy: String,
    pub manual_location_exclusions_json: String,
    pub registered_cloud_locations_json: String,
    pub cloud_detection_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CloudPolicy {
    #[default]
    ExcludeRegisteredRoots,
    IncludeSyncRootsSkipPlaceholders,
    AllowCloudAccess,
}

impl CloudPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExcludeRegisteredRoots => "exclude_registered_roots",
            Self::IncludeSyncRootsSkipPlaceholders => "include_sync_roots_skip_placeholders",
            Self::AllowCloudAccess => "allow_cloud_access",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CloudDetectionStatus {
    Complete,
    Unsupported,
    #[default]
    Unavailable,
}

impl CloudDetectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Unsupported => "unsupported",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredCloudLocation {
    pub path: String,
    pub provider_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunParameters {
    pub roots: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub directory_similarity_threshold_millis: u16,
    #[serde(default)]
    pub cloud_policy: CloudPolicy,
    #[serde(default)]
    pub manual_location_exclusions: Vec<String>,
    #[serde(default)]
    pub registered_cloud_locations: Vec<RegisteredCloudLocation>,
    #[serde(default)]
    pub cloud_detection_status: CloudDetectionStatus,
}

impl RunParameters {
    pub fn from_json(value: &str) -> Option<Self> {
        serde_json::from_str(value).ok()
    }

    pub fn roots_json(&self) -> String {
        serde_json::to_string(&self.roots).unwrap_or_else(|_| "[]".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRun {
    pub id: i64,
    pub session_id: i64,
    pub parameters_json: String,
    pub status: String,
    pub phase: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub files_discovered: i64,
    pub bytes_discovered: i64,
    pub files_hashed: i64,
    pub duplicate_file_groups: i64,
    pub duplicate_folder_groups: i64,
    pub wasted_bytes: i64,
    pub warning_count: i64,
    pub excluded_subtree_count: i64,
    pub error_message: Option<String>,
    pub engine_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunExclusion {
    pub id: i64,
    pub run_id: i64,
    pub path: String,
    pub reason_code: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub occurrence_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunExclusionInsert {
    pub path: String,
    pub reason_code: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub id: i64,
    pub run_id: i64,
    pub root_path: String,
    pub canonical_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub drive_letter: String,
    pub file_size: i64,
    pub last_modified: i64,
    pub partial_hash: Option<i64>,
    pub content_hash: Option<i64>,
    pub file_identity: Option<String>,
    pub warning_message: Option<String>,
    pub marked_deleted: bool,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub id: i64,
    pub run_id: i64,
    pub content_hash: i64,
    pub file_size: i64,
    pub file_count: i64,
    pub wasted_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroupMember {
    pub id: i64,
    pub group_id: i64,
    pub file_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFileGroupSortField {
    RecoverableBytes,
    GroupSize,
    CopyCount,
    RepresentativeName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFileMemberSortField {
    Path,
    ModifiedTime,
    Size,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DuplicateFilePathMatchMode {
    #[default]
    Substring,
    Exact,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DuplicateFileExtensionMatchMode {
    #[default]
    AnyMember,
    AllMembers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageCursorValue {
    Integer(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCursor {
    pub value: PageCursorValue,
    pub id: i64,
    pub before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupFilter {
    pub search: Option<String>,
    pub path_match: DuplicateFilePathMatchMode,
    /// A locale-independently lowercased final filename suffix. `Some("")` explicitly
    /// selects members without an extension; `None` does not filter by extension.
    pub extension_key: Option<String>,
    /// Controls whether any immutable member or every immutable member must have
    /// `extension_key`. Ignored when `extension_key` is `None`.
    pub extension_match: DuplicateFileExtensionMatchMode,
    pub minimum_size: i64,
    pub minimum_copy_count: i64,
    pub across_drives: bool,
    pub selected_root: Option<String>,
    pub selected_drive: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupPageQuery {
    pub run_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFileGroupSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFileGroupFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupResult {
    pub id: i64,
    pub run_id: i64,
    pub file_size: i64,
    pub file_count: i64,
    pub recoverable_bytes: i64,
    pub representative_name: String,
    pub distinct_selected_root_count: i64,
    pub distinct_drive_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileReviewSummary {
    pub matching_group_count: i64,
    pub matching_copy_count: i64,
    pub potential_recoverable_bytes: i64,
    pub largest_recoverable_bytes: i64,
    pub distinct_selected_root_count: i64,
    pub distinct_drive_count: i64,
    pub across_drive_group_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupPage {
    pub groups: Vec<DuplicateFileGroupResult>,
    pub total: i64,
    pub summary: DuplicateFileReviewSummary,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFileSelectedRootFacetSortField {
    MatchingGroupCount,
    Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileSelectedRootFacetPageQuery {
    pub run_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFileSelectedRootFacetSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFileGroupFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileSelectedRootFacetResult {
    pub cursor_id: i64,
    pub value: String,
    pub matching_group_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileSelectedRootFacetPage {
    pub facets: Vec<DuplicateFileSelectedRootFacetResult>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFileDriveFacetSortField {
    MatchingGroupCount,
    Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileDriveFacetPageQuery {
    pub run_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFileDriveFacetSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFileGroupFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileDriveFacetResult {
    pub cursor_id: i64,
    pub value: String,
    pub matching_group_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileDriveFacetPage {
    pub facets: Vec<DuplicateFileDriveFacetResult>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberFilter {
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberPageQuery {
    pub run_id: i64,
    pub group_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFileMemberSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFileMemberFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberResult {
    pub id: i64,
    pub group_id: i64,
    pub canonical_path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub root_path: String,
    pub relative_path: String,
    pub drive_letter: String,
    pub file_size: i64,
    pub last_modified: i64,
    pub review_decision: ReviewDecisionKind,
    pub review_provenance: Option<String>,
    pub review_decided_at: Option<String>,
    pub review_application_id: Option<i64>,
    pub validation_state: Option<String>,
    pub validation_reason_code: Option<String>,
    pub validation_observed_at: Option<String>,
    pub invalidated_decision: Option<ReviewDecisionKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberPage {
    pub members: Vec<DuplicateFileMemberResult>,
    pub total: i64,
    pub has_more: bool,
    pub review_plan_id: Option<i64>,
    pub review_revision: i64,
    pub review_summary: ReviewGroupSummary,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReviewDecisionKind {
    Keep,
    Remove,
    #[default]
    Undecided,
}

impl ReviewDecisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Remove => "remove",
            Self::Undecided => "undecided",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "keep" => Some(Self::Keep),
            "remove" => Some(Self::Remove),
            "undecided" => Some(Self::Undecided),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPlan {
    pub id: i64,
    pub run_id: i64,
    pub state: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewPlanSummary {
    pub decided_group_count: i64,
    pub keep_count: i64,
    pub remove_count: i64,
    pub undecided_count: i64,
    pub decided_folder_group_count: i64,
    pub folder_keep_count: i64,
    pub folder_remove_count: i64,
    pub folder_undecided_count: i64,
    pub effective_removal_file_count: i64,
    pub planned_removal_physical_item_count: i64,
    pub planned_removal_bytes: i64,
    pub remaining_physical_copy_count: i64,
    pub intact_folder_copy_count: i64,
    pub rule_keep_count: i64,
    pub rule_remove_count: i64,
    pub active_rule_application_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewGroupSummary {
    pub group_id: i64,
    pub keep_count: i64,
    pub remove_count: i64,
    pub undecided_count: i64,
    pub remaining_physical_copy_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPlanView {
    pub plan: Option<ReviewPlan>,
    pub summary: ReviewPlanSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewGroupPage {
    pub groups: Vec<ReviewGroupSummary>,
    pub total: i64,
    pub has_more: bool,
    pub plan_id: Option<i64>,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewDecisionMutation {
    pub plan_id: i64,
    pub applied_revision: i64,
    pub replayed: bool,
    pub decision: ReviewDecisionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLiveValidationRequest {
    pub operation_id: String,
    pub run_id: i64,
    pub group_id: i64,
    pub expected_review_revision: i64,
    pub scope: String,
    pub file_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLiveValidationItem {
    pub file_id: i64,
    pub state: String,
    pub reason_code: String,
    pub observed_file_identity: Option<String>,
    pub observed_file_size: Option<i64>,
    pub observed_last_modified: Option<i64>,
    pub os_error: Option<i64>,
    pub decision_invalidated: bool,
    pub invalidated_decision: Option<ReviewDecisionKind>,
    pub observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLiveValidationResult {
    pub validation_id: i64,
    pub run_id: i64,
    pub group_id: i64,
    pub review_revision: i64,
    pub scope: String,
    pub replayed: bool,
    pub items: Vec<ReviewLiveValidationItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewFolderGroupSummary {
    pub folder_group_id: i64,
    pub keep_count: i64,
    pub remove_count: i64,
    pub undecided_count: i64,
    pub intact_copy_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewFolderGroupPage {
    pub groups: Vec<ReviewFolderGroupSummary>,
    pub total: i64,
    pub has_more: bool,
    pub plan_id: Option<i64>,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewFolderDecisionMutation {
    pub plan_id: i64,
    pub applied_revision: i64,
    pub replayed: bool,
    pub decision: ReviewDecisionKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreflightSummary {
    pub logical_removal_count: i64,
    pub physical_removal_count: i64,
    pub folder_removal_count: i64,
    pub affected_group_count: i64,
    pub planned_removal_bytes: i64,
    pub total_item_count: i64,
    pub processed_item_count: i64,
    pub ready_count: i64,
    pub changed_count: i64,
    pub missing_count: i64,
    pub unavailable_count: i64,
    pub conflict_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preflight {
    pub id: i64,
    pub operation_id: String,
    pub run_id: i64,
    pub plan_id: i64,
    pub review_revision: i64,
    pub snapshot_signature: String,
    pub status: String,
    pub summary: PreflightSummary,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightView {
    pub preflight: Preflight,
    pub current_review_revision: i64,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightStartResult {
    pub view: PreflightView,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightItem {
    pub id: i64,
    pub preflight_id: i64,
    pub ordinal: i64,
    pub target_kind: String,
    pub target_role: String,
    pub physical_key: String,
    pub group_id: Option<i64>,
    pub folder_group_id: Option<i64>,
    pub folder_member_id: Option<i64>,
    pub snapshot_file_id: Option<i64>,
    pub snapshot_directory_id: Option<i64>,
    pub snapshot_path: String,
    pub snapshot_file_identity: Option<String>,
    pub snapshot_file_size: Option<i64>,
    pub snapshot_last_modified: Option<i64>,
    pub snapshot_content_hash: Option<i64>,
    pub snapshot_structural_fingerprint: Option<String>,
    pub snapshot_verified_fingerprint: Option<String>,
    pub outcome: String,
    pub reason_code: Option<String>,
    pub observed_file_identity: Option<String>,
    pub observed_file_size: Option<i64>,
    pub observed_last_modified: Option<i64>,
    pub observed_content_hash: Option<i64>,
    pub os_error: Option<i64>,
    pub observed_at: Option<String>,
    pub source_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightItemPage {
    pub items: Vec<PreflightItem>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightObservation {
    pub outcome: String,
    pub reason_code: Option<String>,
    pub observed_file_identity: Option<String>,
    pub observed_file_size: Option<i64>,
    pub observed_last_modified: Option<i64>,
    pub observed_content_hash: Option<i64>,
    pub os_error: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecycleOperationSummary {
    pub logical_removal_count: i64,
    pub shell_item_count: i64,
    pub physical_item_count: i64,
    pub folder_item_count: i64,
    pub affected_group_count: i64,
    pub planned_removal_bytes: i64,
    pub affected_location_count: i64,
    pub exclusion_count: i64,
    pub eligible_count: i64,
    pub non_recyclable_count: i64,
    pub pending_eligibility_count: i64,
    pub recycled_count: i64,
    pub failed_count: i64,
    pub cancelled_count: i64,
    pub unknown_count: i64,
    pub pending_result_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleOperation {
    pub id: i64,
    pub operation_id: String,
    pub run_id: i64,
    pub plan_id: i64,
    pub preflight_id: i64,
    pub review_revision: i64,
    pub preflight_snapshot_signature: String,
    pub intent_signature: String,
    pub policy_version: i64,
    pub status: String,
    pub summary: RecycleOperationSummary,
    pub prepared_at: String,
    pub confirmation_signature: Option<String>,
    pub confirmation_expires_at: Option<String>,
    pub submitted_at: Option<String>,
    pub completed_at: Option<String>,
    pub cancellation_requested: bool,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleOperationView {
    pub operation: RecycleOperation,
    pub current_review_revision: i64,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleOperationMutationResult {
    pub view: RecycleOperationView,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleOperationItem {
    pub id: i64,
    pub recycle_operation_id: i64,
    pub batch_id: i64,
    pub ordinal: i64,
    pub preflight_item_id: i64,
    pub preflight_source_id: Option<i64>,
    pub target_kind: String,
    pub physical_key: String,
    pub snapshot_path: String,
    pub group_id: Option<i64>,
    pub folder_group_id: Option<i64>,
    pub folder_member_id: Option<i64>,
    pub snapshot_file_id: Option<i64>,
    pub snapshot_directory_id: Option<i64>,
    pub snapshot_file_identity: Option<String>,
    pub snapshot_file_size: Option<i64>,
    pub snapshot_last_modified: Option<i64>,
    pub planned_bytes: i64,
    pub eligibility_status: String,
    pub eligibility_code: Option<String>,
    pub result_status: String,
    pub result_code: Option<String>,
    pub shell_hresult: Option<i64>,
    pub recycled_item_present: Option<bool>,
    pub result_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleOperationItemPage {
    pub items: Vec<RecycleOperationItem>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleOperationBatch {
    pub id: i64,
    pub recycle_operation_id: i64,
    pub ordinal: i64,
    pub item_signature: String,
    pub status: String,
    pub admission_expires_at: Option<String>,
    pub shell_attempt_id: Option<String>,
    pub started_at: Option<String>,
    pub reported_at: Option<String>,
    pub items: Vec<RecycleOperationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleEligibilityObservation {
    pub item_id: i64,
    pub status: String,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecycleItemResultObservation {
    pub item_id: i64,
    pub status: String,
    pub reason_code: Option<String>,
    pub shell_hresult: Option<i64>,
    pub recycled_item_present: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryObservationKind {
    ObservedInRecycleBin,
    ObservedAtSource,
    ObservedInBoth,
    ObservedInNeither,
    DeferredUnresolved,
}

impl RecoveryObservationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ObservedInRecycleBin => "observed_in_recycle_bin",
            Self::ObservedAtSource => "observed_at_source",
            Self::ObservedInBoth => "observed_in_both",
            Self::ObservedInNeither => "observed_in_neither",
            Self::DeferredUnresolved => "deferred_unresolved",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "observed_in_recycle_bin" => Some(Self::ObservedInRecycleBin),
            "observed_at_source" => Some(Self::ObservedAtSource),
            "observed_in_both" => Some(Self::ObservedInBoth),
            "observed_in_neither" => Some(Self::ObservedInNeither),
            "deferred_unresolved" => Some(Self::DeferredUnresolved),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryReviewState {
    NotStarted,
    InProgress,
    ReviewCompleteWithUnresolvedEvidence,
}

impl RecoveryReviewState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::InProgress => "in_progress",
            Self::ReviewCompleteWithUnresolvedEvidence => {
                "review_complete_with_unresolved_evidence"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReviewSummary {
    pub recycle_operation_id: i64,
    pub state: RecoveryReviewState,
    pub unknown_item_count: i64,
    pub observed_item_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReviewObservation {
    pub id: i64,
    pub request_id: String,
    pub recycle_operation_id: i64,
    pub item_id: i64,
    pub observation: RecoveryObservationKind,
    pub observed_at: String,
    pub note: Option<String>,
    pub evidence_version: i64,
    pub supersedes_observation_id: Option<i64>,
    pub correction_reason: Option<String>,
    pub created_at: String,
    pub superseded_by_observation_id: Option<i64>,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReviewObservationInput {
    pub request_id: String,
    pub recycle_operation_id: i64,
    pub item_id: i64,
    pub observation: RecoveryObservationKind,
    pub observed_at: String,
    pub note: Option<String>,
    pub evidence_version: i64,
    pub supersedes_observation_id: Option<i64>,
    pub correction_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReviewObservationPage {
    pub observations: Vec<RecoveryReviewObservation>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReviewMutationResult {
    pub summary: RecoveryReviewSummary,
    pub observation: RecoveryReviewObservation,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceRule {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub state: String,
    pub revision: i64,
    pub roots: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceRuleSummary {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub revision: i64,
    pub root_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceRuleSaveResult {
    pub rule: PreferenceRule,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferencePreviewScope {
    SelectedSets(Vec<i64>),
    CurrentFilter(DuplicateFileGroupFilter),
    CompletedRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencePreviewStatus {
    Applicable,
    Blocked,
}

impl PreferencePreviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applicable => "applicable",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferencePreviewGroup {
    pub group_id: i64,
    pub status: PreferencePreviewStatus,
    pub best_rank: Option<i64>,
    pub preferred_root: Option<String>,
    pub tied_preferred_path_count: i64,
    pub proposed_keep_path_count: i64,
    pub proposed_remove_path_count: i64,
    pub proposed_remove_physical_item_count: i64,
    pub proposed_remove_bytes: i64,
    pub manual_keep_count: i64,
    pub manual_remove_count: i64,
    pub explanation_code: String,
    pub conflict_file_id: Option<i64>,
    pub conflict_folder_member_id: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreferencePreviewSummary {
    pub scoped_group_count: i64,
    pub scoped_logical_path_count: i64,
    pub scoped_physical_item_count: i64,
    pub scoped_bytes: i64,
    pub affected_group_count: i64,
    pub blocked_group_count: i64,
    pub proposed_keep_path_count: i64,
    pub proposed_remove_path_count: i64,
    pub proposed_remove_physical_item_count: i64,
    pub proposed_remove_bytes: i64,
    pub manual_keep_path_count: i64,
    pub manual_remove_path_count: i64,
    pub tied_group_count: i64,
    pub no_ranked_root_group_count: i64,
    pub missing_rule_root_count: i64,
    pub overlap_conflict_count: i64,
    pub file_survivor_conflict_count: i64,
    pub folder_survivor_conflict_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferencePreviewPage {
    pub groups: Vec<PreferencePreviewGroup>,
    pub total: i64,
    pub has_more: bool,
    pub rule_id: i64,
    pub rule_revision: i64,
    pub review_plan_id: Option<i64>,
    pub review_revision: i64,
    pub preview_signature: String,
    pub summary: PreferencePreviewSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreferenceApplicationSummary {
    pub scoped_group_count: i64,
    pub applicable_group_count: i64,
    pub blocked_group_count: i64,
    pub rule_keep_path_count: i64,
    pub rule_remove_path_count: i64,
    pub rule_remove_physical_item_count: i64,
    pub rule_remove_bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceRuleApplication {
    pub id: i64,
    pub plan_id: i64,
    pub run_id: i64,
    pub rule_id: i64,
    pub rule_revision: i64,
    pub rule_name: String,
    pub rule_kind: String,
    pub rule_roots: Vec<String>,
    pub scope_kind: String,
    pub scope_json: String,
    pub scope_signature: String,
    pub preview_signature: String,
    pub source_review_revision: i64,
    pub applied_revision: i64,
    pub state: String,
    pub created_at: String,
    pub reversed_at: Option<String>,
    pub summary: PreferenceApplicationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceRuleApplicationResult {
    pub application: PreferenceRuleApplication,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceRuleApplicationPage {
    pub applications: Vec<PreferenceRuleApplication>,
    pub total: i64,
    pub has_more: bool,
    pub plan_id: Option<i64>,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceRuleReversalResult {
    pub application_id: i64,
    pub plan_id: i64,
    pub applied_revision: i64,
    pub replayed: bool,
    pub removed_keep_count: i64,
    pub removed_remove_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFolderGroupSortField {
    TotalBytes,
    CopyCount,
    FileCount,
    RepresentativePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFolderMemberSortField {
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupFilter {
    pub search: Option<String>,
    pub minimum_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupPageQuery {
    pub run_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFolderGroupSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFolderGroupFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupResult {
    pub id: i64,
    pub run_id: i64,
    pub total_size: i64,
    pub file_count: i64,
    pub folder_count: i64,
    pub representative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupPage {
    pub groups: Vec<DuplicateFolderGroupResult>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberFilter {
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberPageQuery {
    pub run_id: i64,
    pub group_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFolderMemberSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFolderMemberFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberResult {
    pub id: i64,
    pub group_id: i64,
    pub path: String,
    pub review_decision: ReviewDecisionKind,
    pub review_provenance: Option<String>,
    pub review_decided_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberPage {
    pub members: Vec<DuplicateFolderMemberResult>,
    pub total: i64,
    pub has_more: bool,
    pub review_plan_id: Option<i64>,
    pub review_revision: i64,
    pub review_summary: ReviewFolderGroupSummary,
}

#[derive(Debug, Clone)]
pub struct ExactFolderGroupInsert {
    pub structural_fingerprint: String,
    pub verified_fingerprint: String,
    pub total_size: i64,
    pub file_count: i64,
    pub directory_ids: Vec<i64>,
    pub is_suppressed: bool,
}

#[derive(Debug, Clone)]
pub struct DirectoryNode {
    pub id: i64,
    pub run_id: i64,
    pub path: String,
    pub name: String,
    pub parent_id: Option<i64>,
    pub total_size: i64,
    pub file_count: i64,
    pub depth: i64,
}

#[derive(Debug, Clone)]
pub struct DirectoryFingerprint {
    pub id: i64,
    pub directory_id: i64,
    pub content_fingerprint: String,
    pub file_hash_set: String,
}

#[derive(Debug, Clone)]
pub struct DirectorySimilarity {
    pub id: i64,
    pub run_id: i64,
    pub dir_a_id: i64,
    pub dir_b_id: i64,
    pub dir_a_path: String,
    pub dir_b_path: String,
    pub similarity_score: f64,
    pub shared_bytes: i64,
    pub match_type: String,
}

#[derive(Debug, Clone)]
pub struct DeletionPlanEntry {
    pub id: i64,
    pub file_id: i64,
    pub marked_at: String,
    pub strategy: Option<String>,
    pub executed_at: Option<String>,
    pub execution_result: Option<String>,
}
