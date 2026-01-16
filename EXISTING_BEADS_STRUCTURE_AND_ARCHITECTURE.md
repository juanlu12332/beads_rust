# Existing Beads Structure and Architecture

> Comprehensive specification of the Go beads codebase for porting to Rust.
> This document serves as the complete reference for the Rust port - consult this instead of Go source files.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Directory Structure](#2-directory-structure)
3. [Data Types and Models](#3-data-types-and-models)
4. [Storage Interface Specification](#4-storage-interface-specification)
5. [SQLite Storage Implementation](#5-sqlite-storage-implementation)
6. [CLI Commands Specification](#6-cli-commands-specification)
7. [JSONL Import/Export System](#7-jsonl-importexport-system)
8. [Ready/Blocked Logic and Dependency Graph](#8-readyblocked-logic-and-dependency-graph)
9. [Configuration System](#9-configuration-system)
10. [Validation Rules](#10-validation-rules)
11. [ID Generation and Content Hashing](#11-id-generation-and-content-hashing)
12. [Key Architectural Patterns](#12-key-architectural-patterns)
13. [Error Handling](#13-error-handling)
14. [Porting Considerations](#14-porting-considerations)
15. [Additional Legacy Findings (2026-01-16)](#15-additional-legacy-findings-2026-01-16)

---

## 1. Project Overview

**Location:** `./legacy_beads/` (gitignored reference copy)

**Statistics:**
- ~267,622 lines of Go code
- ~100 files in SQLite storage alone
- 40 database migrations
- 15+ CLI commands with extensive flag sets
- 62+ storage interface methods

**Core Architecture:**
- SQLite + JSONL hybrid storage
- Optional daemon mode with RPC (not porting initially)
- Content-addressable issues with hash-based IDs
- Git-integrated synchronization
- Non-invasive design philosophy (no auto git hooks, no daemon required)

**Key Design Principles (br vs bd):**
- **No automatic git hooks** — Users add hooks manually if desired
- **No automatic git operations** — No auto-commit, no auto-push
- **No daemon/RPC** — Simple CLI only, no background processes
- **Explicit over implicit** — Every git operation requires explicit user command

---

## 2. Directory Structure

```
legacy_beads/
├── beads.go                    # Package root, version info
├── cmd/
│   └── bd/                     # CLI entry point (~100 files)
│       ├── main.go             # Entry point, Cobra root command
│       ├── create.go           # Issue creation
│       ├── update.go           # Issue updates
│       ├── close.go            # Issue closing
│       ├── list.go             # Issue listing
│       ├── show.go             # Issue details
│       ├── ready.go            # Ready work queries
│       ├── blocked.go          # Blocked issues queries
│       ├── dep.go              # Dependency management
│       ├── label.go            # Label management
│       ├── search.go           # Full-text search
│       ├── stats.go            # Statistics
│       ├── sync.go             # Git synchronization
│       ├── config.go           # Configuration
│       ├── init.go             # Initialize workspace
│       ├── export.go           # JSONL export
│       ├── import.go           # JSONL import
│       ├── autoflush.go        # Auto-export logic
│       ├── autoimport.go       # Auto-import logic
│       ├── daemon*.go          # Daemon mode (SKIP)
│       └── ...
├── internal/
│   ├── types/                  # Core data types
│   │   ├── types.go            # Issue, Dependency, etc. (42KB)
│   │   ├── id_generator.go     # Hash-based ID generation
│   │   ├── lock.go             # Lock types
│   │   └── validation.go       # Validation helpers
│   ├── storage/
│   │   ├── storage.go          # Storage interface (10KB, 62+ methods)
│   │   ├── sqlite/             # SQLite implementation (PORT THIS)
│   │   │   ├── store.go        # Main storage struct
│   │   │   ├── schema.go       # Database schema
│   │   │   ├── queries.go      # SQL queries
│   │   │   ├── issues.go       # Issue CRUD
│   │   │   ├── dependencies.go # Dependency operations
│   │   │   ├── labels.go       # Label operations
│   │   │   ├── comments.go     # Comment operations
│   │   │   ├── events.go       # Event/audit operations
│   │   │   ├── ready.go        # Ready work queries
│   │   │   ├── blocked.go      # Blocked cache management
│   │   │   ├── dirty_issues.go # Dirty tracking
│   │   │   ├── config.go       # Config storage
│   │   │   ├── metadata.go     # Metadata storage
│   │   │   ├── export_hashes.go# Export hash tracking
│   │   │   ├── migrations/     # 40 migrations
│   │   │   │   ├── 001_dirty_issues_table.sql
│   │   │   │   ├── ...
│   │   │   │   └── 040_quality_score_column.sql
│   │   │   └── ...
│   │   ├── dolt/               # Dolt backend (DO NOT PORT)
│   │   ├── memory/             # In-memory backend (for testing)
│   │   └── factory/            # Backend factory
│   ├── export/                 # JSONL export logic
│   ├── autoimport/             # Auto-import from JSONL
│   ├── importer/               # Import logic with collision detection
│   ├── compact/                # JSONL compaction
│   ├── configfile/             # Configuration file handling
│   ├── validation/             # Input validation
│   ├── hooks/                  # Hook system (SKIP for br)
│   ├── git/                    # Git integration
│   ├── rpc/                    # RPC daemon (SKIP initially)
│   ├── linear/                 # Linear.app integration (SKIP)
│   └── ui/                     # Terminal UI helpers
└── docs/                       # Documentation
```

---

## 3. Data Types and Models

### 3.1 Issue Struct (Primary Entity)

The `Issue` struct is the primary data entity. For the Rust port, we exclude Gastown-specific fields (agent, molecule, gate, rig, convoy, HOP features).

**Fields to Port:**

```go
type Issue struct {
    // === Core Identification ===
    ID          string `json:"id"`           // Hash-based ID (e.g., "bd-abc123")
    ContentHash string `json:"-"`            // SHA256, NOT exported to JSONL

    // === Content Fields ===
    Title              string `json:"title"`                         // Required, max 500 chars
    Description        string `json:"description,omitempty"`
    Design             string `json:"design,omitempty"`
    AcceptanceCriteria string `json:"acceptance_criteria,omitempty"`
    Notes              string `json:"notes,omitempty"`

    // === Status & Workflow ===
    Status    Status    `json:"status,omitempty"`      // open, in_progress, blocked, closed, etc.
    Priority  int       `json:"priority"`              // 0-4 (P0-P4), NO omitempty (0 is valid)
    IssueType IssueType `json:"issue_type,omitempty"`  // task, bug, feature, epic, etc.

    // === Assignment ===
    Assignee         string `json:"assignee,omitempty"`
    Owner            string `json:"owner,omitempty"`           // Git author email for attribution
    EstimatedMinutes *int   `json:"estimated_minutes,omitempty"`

    // === Timestamps ===
    CreatedAt       time.Time  `json:"created_at"`
    CreatedBy       string     `json:"created_by,omitempty"`
    UpdatedAt       time.Time  `json:"updated_at"`
    ClosedAt        *time.Time `json:"closed_at,omitempty"`
    CloseReason     string     `json:"close_reason,omitempty"`
    ClosedBySession string     `json:"closed_by_session,omitempty"`  // Claude Code session ID

    // === Time-Based Scheduling ===
    DueAt      *time.Time `json:"due_at,omitempty"`       // When issue should complete
    DeferUntil *time.Time `json:"defer_until,omitempty"`  // Hide from bd ready until

    // === External Integration ===
    ExternalRef  *string `json:"external_ref,omitempty"`   // e.g., "gh-9", "jira-ABC"
    SourceSystem string  `json:"source_system,omitempty"`  // Federation source identifier

    // === Compaction Metadata ===
    CompactionLevel   int        `json:"compaction_level,omitempty"`   // 0=none, 1=minor, 2=major
    CompactedAt       *time.Time `json:"compacted_at,omitempty"`
    CompactedAtCommit *string    `json:"compacted_at_commit,omitempty"`
    OriginalSize      int        `json:"original_size,omitempty"`       // Bytes before compaction

    // === Internal Routing (NOT exported to JSONL) ===
    SourceRepo     string `json:"-"`  // Which repo owns this issue
    IDPrefix       string `json:"-"`  // Override prefix for ID generation
    PrefixOverride string `json:"-"`  // Replace config prefix entirely

    // === Relational Data ===
    Labels       []string      `json:"labels,omitempty"`
    Dependencies []*Dependency `json:"dependencies,omitempty"`
    Comments     []*Comment    `json:"comments,omitempty"`

    // === Soft-Delete (Tombstone) ===
    DeletedAt    *time.Time `json:"deleted_at,omitempty"`
    DeletedBy    string     `json:"deleted_by,omitempty"`
    DeleteReason string     `json:"delete_reason,omitempty"`
    OriginalType string     `json:"original_type,omitempty"`  // Type before deletion

    // === Messaging/Ephemeral ===
    Sender    string `json:"sender,omitempty"`     // For message-type issues
    Ephemeral bool   `json:"ephemeral,omitempty"`  // If true, not exported to JSONL

    // === Context Markers ===
    Pinned     bool `json:"pinned,omitempty"`      // Persistent context marker
    IsTemplate bool `json:"is_template,omitempty"` // Read-only template
}
```

**Fields NOT to Port (Gastown features):**

```go
// DO NOT PORT - Agent Identity Fields
HookBead     string     `json:"hook_bead,omitempty"`
RoleBead     string     `json:"role_bead,omitempty"`
AgentState   AgentState `json:"agent_state,omitempty"`
LastActivity *time.Time `json:"last_activity,omitempty"`
RoleType     string     `json:"role_type,omitempty"`
Rig          string     `json:"rig,omitempty"`

// DO NOT PORT - Molecule/Work Type
MolType  MolType  `json:"mol_type,omitempty"`
WorkType WorkType `json:"work_type,omitempty"`

// DO NOT PORT - Gate Fields (Async Coordination)
AwaitType string        `json:"await_type,omitempty"`
AwaitID   string        `json:"await_id,omitempty"`
Timeout   time.Duration `json:"timeout,omitempty"`
Waiters   []string      `json:"waiters,omitempty"`
Holder    string        `json:"holder,omitempty"`

// DO NOT PORT - HOP Fields (Entity Tracking)
Creator      *EntityRef   `json:"creator,omitempty"`
Validations  []Validation `json:"validations,omitempty"`
QualityScore *float32     `json:"quality_score,omitempty"`
Crystallizes bool         `json:"crystallizes,omitempty"`

// DO NOT PORT - Event Fields
EventKind string `json:"event_kind,omitempty"`
Actor     string `json:"actor,omitempty"`
Target    string `json:"target,omitempty"`
Payload   string `json:"payload,omitempty"`

// DO NOT PORT - Bonding (Compound Molecules)
BondedFrom []BondRef `json:"bonded_from,omitempty"`
```

### 3.2 Status Enum

```go
const (
    StatusOpen       Status = "open"        // Default status for new issues
    StatusInProgress Status = "in_progress" // Work has begun
    StatusBlocked    Status = "blocked"     // Explicitly marked blocked (manual)
    StatusDeferred   Status = "deferred"    // Postponed for later
    StatusClosed     Status = "closed"      // Complete
    StatusTombstone  Status = "tombstone"   // Soft-deleted, preserved for history
    StatusPinned     Status = "pinned"      // Persistent context marker
    StatusHooked     Status = "hooked"      // Attached to agent's hook (Gastown - optional)
)

// ValidStatuses returns all valid status values
func ValidStatuses() []Status {
    return []Status{
        StatusOpen, StatusInProgress, StatusBlocked,
        StatusDeferred, StatusClosed, StatusTombstone, StatusPinned,
    }
}

// IsTerminal returns true if the status represents a completed state
func (s Status) IsTerminal() bool {
    return s == StatusClosed || s == StatusTombstone
}

// IsActive returns true if the status represents active work
func (s Status) IsActive() bool {
    return s == StatusOpen || s == StatusInProgress
}
```

**Status Transitions:**

```
                    ┌─────────────────────────────────────┐
                    │                                     │
                    v                                     │
    ┌──────┐     ┌─────────────┐     ┌────────┐          │
    │ open │────▶│ in_progress │────▶│ closed │          │
    └──────┘     └─────────────┘     └────────┘          │
       │               │                  │              │
       │               │                  │              │
       v               v                  v              │
    ┌─────────┐   ┌─────────┐      ┌───────────┐        │
    │ blocked │   │ deferred │     │ tombstone │        │
    └─────────┘   └─────────┘      └───────────┘        │
       │               │                                 │
       └───────────────┴─────────────────────────────────┘
                    (reopen)
```

### 3.3 IssueType Enum

**Types to Port:**

```go
const (
    TypeBug     IssueType = "bug"      // Defect to fix
    TypeFeature IssueType = "feature"  // New functionality
    TypeTask    IssueType = "task"     // Generic work item (default)
    TypeEpic    IssueType = "epic"     // Container for related issues
    TypeChore   IssueType = "chore"    // Maintenance/housekeeping
    TypeDocs    IssueType = "docs"     // Documentation
    TypeQuestion IssueType = "question" // Question/discussion
)

// Default type for new issues
const DefaultIssueType = TypeTask
```

**Types NOT to Port (Gastown):**

```go
// DO NOT PORT
TypeMessage      IssueType = "message"       // Ephemeral inter-worker
TypeMergeRequest IssueType = "merge-request"
TypeMolecule     IssueType = "molecule"      // Template for hierarchies
TypeGate         IssueType = "gate"          // Async coordination
TypeAgent        IssueType = "agent"         // Agent identity
TypeRole         IssueType = "role"          // Agent role definition
TypeRig          IssueType = "rig"           // Multi-repo workspace
TypeConvoy       IssueType = "convoy"        // Cross-project tracking
TypeEvent        IssueType = "event"         // Operational state change
TypeSlot         IssueType = "slot"          // Exclusive access
```

### 3.4 Dependency Struct

```go
type Dependency struct {
    IssueID     string         `json:"issue_id"`              // The issue that has the dependency
    DependsOnID string         `json:"depends_on_id"`         // The issue being depended on
    Type        DependencyType `json:"type"`                  // Relationship type
    CreatedAt   time.Time      `json:"created_at"`
    CreatedBy   string         `json:"created_by,omitempty"`
    Metadata    string         `json:"metadata,omitempty"`    // Type-specific JSON blob
    ThreadID    string         `json:"thread_id,omitempty"`   // Conversation threading
}
```

### 3.5 DependencyType Enum

**Types to Port:**

```go
const (
    // === Workflow Types (affect ready work calculation) ===
    DepBlocks            DependencyType = "blocks"             // A blocks B = B depends on A
    DepParentChild       DependencyType = "parent-child"       // Hierarchical relationship
    DepConditionalBlocks DependencyType = "conditional-blocks" // Blocks only if condition met
    DepWaitsFor          DependencyType = "waits-for"          // Soft block, waits for children

    // === Association Types (informational only) ===
    DepRelated        DependencyType = "related"          // Soft link for reference
    DepDiscoveredFrom DependencyType = "discovered-from"  // Found during work on parent

    // === Graph Link Types (informational) ===
    DepRepliesTo  DependencyType = "replies-to"   // Conversation threading
    DepRelatesTo  DependencyType = "relates-to"   // Bidirectional reference
    DepDuplicates DependencyType = "duplicates"   // Marks as duplicate of
    DepSupersedes DependencyType = "supersedes"   // Replaces another issue

    // === Reference Types ===
    DepCausedBy DependencyType = "caused-by"  // Root cause linkage
)

// AffectsReadyWork returns true for dependency types that block ready work
func (t DependencyType) AffectsReadyWork() bool {
    switch t {
    case DepBlocks, DepParentChild, DepConditionalBlocks, DepWaitsFor:
        return true
    default:
        return false
    }
}

// IsBlocking returns true for types that can create blocking relationships
func (t DependencyType) IsBlocking() bool {
    return t == DepBlocks || t == DepParentChild || t == DepConditionalBlocks
}
```

**Types NOT to Port (Gastown):**

```go
// DO NOT PORT - HOP Entity Types
DepAuthoredBy  DependencyType = "authored-by"
DepAssignedTo  DependencyType = "assigned-to"
DepApprovedBy  DependencyType = "approved-by"
DepAttests     DependencyType = "attests"
DepTracks      DependencyType = "tracks"
DepUntil       DependencyType = "until"
DepValidates   DependencyType = "validates"
DepDelegatedFrom DependencyType = "delegated-from"
```

### 3.6 Comment Struct

```go
type Comment struct {
    ID        int64     `json:"id"`         // Auto-increment ID
    IssueID   string    `json:"issue_id"`   // Parent issue
    Author    string    `json:"author"`     // Who wrote the comment
    Text      string    `json:"text"`       // Markdown content
    CreatedAt time.Time `json:"created_at"`
}
```

### 3.7 Event Struct (Audit Trail)

```go
type Event struct {
    ID        int64     `json:"id"`
    IssueID   string    `json:"issue_id"`
    EventType EventType `json:"event_type"`
    Actor     string    `json:"actor"`                // Who performed the action
    OldValue  *string   `json:"old_value,omitempty"`  // Previous value (JSON)
    NewValue  *string   `json:"new_value,omitempty"`  // New value (JSON)
    Comment   *string   `json:"comment,omitempty"`    // Optional description
    CreatedAt time.Time `json:"created_at"`
}

// EventType constants
const (
    EventCreated           EventType = "created"
    EventUpdated           EventType = "updated"
    EventStatusChanged     EventType = "status_changed"
    EventPriorityChanged   EventType = "priority_changed"
    EventAssigneeChanged   EventType = "assignee_changed"
    EventCommented         EventType = "commented"
    EventClosed            EventType = "closed"
    EventReopened          EventType = "reopened"
    EventDependencyAdded   EventType = "dependency_added"
    EventDependencyRemoved EventType = "dependency_removed"
    EventLabelAdded        EventType = "label_added"
    EventLabelRemoved      EventType = "label_removed"
    EventCompacted         EventType = "compacted"
    EventDeleted           EventType = "deleted"           // Soft delete
    EventRestored          EventType = "restored"          // Restored from tombstone
)
```

### 3.8 Statistics Struct

```go
type Statistics struct {
    TotalIssues             int     `json:"total_issues"`
    OpenIssues              int     `json:"open_issues"`
    InProgressIssues        int     `json:"in_progress_issues"`
    ClosedIssues            int     `json:"closed_issues"`
    BlockedIssues           int     `json:"blocked_issues"`
    DeferredIssues          int     `json:"deferred_issues"`
    ReadyIssues             int     `json:"ready_issues"`
    TombstoneIssues         int     `json:"tombstone_issues"`
    PinnedIssues            int     `json:"pinned_issues"`
    EpicsEligibleForClosure int     `json:"epics_eligible_for_closure"`
    AverageLeadTime         float64 `json:"average_lead_time_hours"`

    // Breakdown by type
    ByType     map[string]int `json:"by_type,omitempty"`
    ByPriority map[int]int    `json:"by_priority,omitempty"`
    ByAssignee map[string]int `json:"by_assignee,omitempty"`
}
```

### 3.9 IssueFilter Struct

```go
type IssueFilter struct {
    // === Basic Filters ===
    Status      string   `json:"status,omitempty"`        // Single status
    Statuses    []string `json:"statuses,omitempty"`      // Multiple statuses (OR)
    Priority    *int     `json:"priority,omitempty"`      // Single priority
    Priorities  []int    `json:"priorities,omitempty"`    // Multiple priorities (OR)
    IssueType   string   `json:"issue_type,omitempty"`    // Single type
    IssueTypes  []string `json:"issue_types,omitempty"`   // Multiple types (OR)

    // === Assignment ===
    Assignee   string `json:"assignee,omitempty"`   // Filter by assignee
    Unassigned bool   `json:"unassigned,omitempty"` // Only unassigned issues

    // === Labels ===
    Label     string   `json:"label,omitempty"`      // Single label (exact match)
    Labels    []string `json:"labels,omitempty"`     // All labels must match (AND)
    LabelAny  []string `json:"label_any,omitempty"`  // Any label matches (OR)

    // === Search ===
    Query string `json:"query,omitempty"` // Full-text search in title/description

    // === Date Ranges ===
    CreatedAfter  *time.Time `json:"created_after,omitempty"`
    CreatedBefore *time.Time `json:"created_before,omitempty"`
    UpdatedAfter  *time.Time `json:"updated_after,omitempty"`
    UpdatedBefore *time.Time `json:"updated_before,omitempty"`
    ClosedAfter   *time.Time `json:"closed_after,omitempty"`
    ClosedBefore  *time.Time `json:"closed_before,omitempty"`

    // === Content Presence ===
    HasDescription *bool `json:"has_description,omitempty"`
    HasNotes       *bool `json:"has_notes,omitempty"`
    HasComments    *bool `json:"has_comments,omitempty"`

    // === Special Filters ===
    IncludeTombstones  bool `json:"include_tombstones,omitempty"`
    IncludeEphemeral   bool `json:"include_ephemeral,omitempty"`
    Overdue            bool `json:"overdue,omitempty"`             // due_at < now
    DeferredOnly       bool `json:"deferred_only,omitempty"`
    PinnedOnly         bool `json:"pinned_only,omitempty"`

    // === Exclusions ===
    ExcludeStatuses []string `json:"exclude_statuses,omitempty"`
    ExcludeTypes    []string `json:"exclude_types,omitempty"`
    ExcludeIDs      []string `json:"exclude_ids,omitempty"`

    // === Hierarchy ===
    ParentID string `json:"parent_id,omitempty"` // Direct children only

    // === Pagination ===
    Limit  int `json:"limit,omitempty"`
    Offset int `json:"offset,omitempty"`

    // === Sorting ===
    SortBy    string `json:"sort_by,omitempty"`    // Field to sort by
    SortOrder string `json:"sort_order,omitempty"` // "asc" or "desc"
}
```

---

## 4. Storage Interface Specification

The storage interface defines all operations on the issue database. The Rust port must implement all these methods.

### 4.1 Issue CRUD Operations

```go
// CreateIssue creates a new issue and returns the created issue with ID populated
// - Generates hash-based ID if not provided
// - Sets CreatedAt/UpdatedAt to current time
// - Computes and stores ContentHash
// - Creates "created" event in audit trail
// - Marks issue as dirty for export
// Returns: Created issue with all fields populated, or error
CreateIssue(ctx context.Context, issue *types.Issue) (*types.Issue, error)

// GetIssue retrieves a single issue by exact ID
// - Returns ErrNotFound if issue doesn't exist
// - Populates Labels, Dependencies, Comments if available
// - Does NOT return tombstones unless explicitly requested
GetIssue(ctx context.Context, id string) (*types.Issue, error)

// GetIssueByPrefix retrieves issue by ID prefix (for short ID lookup)
// - First tries exact match, then prefix match
// - Returns ErrNotFound if no match or multiple matches
// - Excludes tombstones from prefix matching
GetIssueByPrefix(ctx context.Context, prefix string) (*types.Issue, error)

// UpdateIssue updates an existing issue
// - Updates UpdatedAt timestamp automatically
// - Recomputes ContentHash
// - Creates appropriate event(s) in audit trail
// - Marks issue as dirty for export
// - Returns ErrNotFound if issue doesn't exist
UpdateIssue(ctx context.Context, issue *types.Issue) error

// CloseIssue closes an issue with optional reason
// - Sets Status to "closed"
// - Sets ClosedAt to current time
// - Optionally sets CloseReason
// - Creates "closed" event
// - Marks as dirty
// - Returns ErrNotFound if issue doesn't exist
CloseIssue(ctx context.Context, id string, reason string) error

// ReopenIssue reopens a closed issue
// - Sets Status to "open"
// - Clears ClosedAt
// - Creates "reopened" event
// - Returns error if issue is tombstone
ReopenIssue(ctx context.Context, id string) error

// DeleteIssue soft-deletes an issue (creates tombstone)
// - Sets Status to "tombstone"
// - Sets DeletedAt, DeletedBy, DeleteReason
// - Preserves OriginalType
// - Creates "deleted" event
// - Marks as dirty (tombstones ARE exported)
DeleteIssue(ctx context.Context, id string, deletedBy string, reason string) error

// HardDeleteIssue permanently removes an issue from database
// - Used only for ephemeral issues (wisps) that were never exported
// - Cascades to dependencies, labels, comments, events
// - Does NOT mark as dirty (nothing to export)
// - Use with extreme caution
HardDeleteIssue(ctx context.Context, id string) error

// RestoreIssue restores a tombstoned issue
// - Sets Status back to OriginalType (or "open" if not set)
// - Clears tombstone fields
// - Creates "restored" event
RestoreIssue(ctx context.Context, id string) error
```

### 4.2 Issue Query Operations

```go
// ListIssues retrieves issues matching filter criteria
// - Returns slice of issues (may be empty)
// - Does NOT populate Dependencies/Comments (use GetIssue for full data)
// - Respects IncludeTombstones flag
// - Applies all filter conditions with AND logic
// - Applies pagination (Limit/Offset)
ListIssues(ctx context.Context, filter *types.IssueFilter) ([]*types.Issue, error)

// SearchIssues performs full-text search on title and description
// - Uses SQLite FTS5 if available, falls back to LIKE
// - Returns issues ordered by relevance
// - Respects status filter (excludes tombstones by default)
SearchIssues(ctx context.Context, query string, filter *types.IssueFilter) ([]*types.Issue, error)

// CountIssues returns count of issues matching filter
// - More efficient than ListIssues when only count needed
CountIssues(ctx context.Context, filter *types.IssueFilter) (int, error)

// GetAllIssues retrieves all issues for export
// - Includes tombstones
// - Excludes ephemeral issues
// - Populates all related data (dependencies, labels, comments)
// - Used by JSONL export
GetAllIssues(ctx context.Context) ([]*types.Issue, error)

// GetIssuesByIDs retrieves multiple issues by ID
// - More efficient than multiple GetIssue calls
// - Returns map[id]*Issue
// - Missing IDs are simply not in the returned map
GetIssuesByIDs(ctx context.Context, ids []string) (map[string]*types.Issue, error)
```

### 4.3 Ready Work Operations

```go
// GetReadyWork retrieves issues ready to be worked on
// - Status must be "open" or "in_progress"
// - NOT in blocked_issues_cache
// - NOT deferred (defer_until is null or in the past)
// - NOT pinned
// - NOT ephemeral
// - Ordered by priority (asc), then created_at (asc)
// Returns: Slice of ready issues
GetReadyWork(ctx context.Context, filter *types.IssueFilter) ([]*types.Issue, error)

// GetBlockedIssues retrieves all issues that are blocked
// - Returns issues in blocked_issues_cache
// - Includes blocking reason (what's blocking each issue)
GetBlockedIssues(ctx context.Context) ([]*types.BlockedIssue, error)

// IsBlocked checks if a specific issue is blocked
// - Checks blocked_issues_cache
// - More efficient than GetBlockedIssues for single check
IsBlocked(ctx context.Context, id string) (bool, error)

// GetBlockingIssues returns issues that block the given issue
// - Returns the immediate blockers (not transitive)
// - Includes dependency type information
GetBlockingIssues(ctx context.Context, id string) ([]*types.Issue, error)

// RefreshBlockedCache rebuilds the blocked_issues_cache table
// - Called after dependency changes or status changes
// - Computes transitive closure of blocking relationships
// - Uses recursive CTE with depth limit
RefreshBlockedCache(ctx context.Context) error
```

### 4.4 Dependency Operations

```go
// AddDependency creates a dependency relationship
// - Validates both issues exist
// - Checks for cycles (returns ErrCycle if detected)
// - Creates "dependency_added" event
// - Marks both issues as dirty
// - Triggers blocked cache refresh
AddDependency(ctx context.Context, dep *types.Dependency) error

// RemoveDependency removes a dependency relationship
// - Returns error if dependency doesn't exist
// - Creates "dependency_removed" event
// - Marks both issues as dirty
// - Triggers blocked cache refresh
RemoveDependency(ctx context.Context, issueID, dependsOnID string) error

// GetDependencies retrieves dependencies for an issue
// - direction "down": things this issue depends on
// - direction "up": things that depend on this issue
// - direction "both": all dependencies
GetDependencies(ctx context.Context, issueID string, direction string) ([]*types.Dependency, error)

// GetDependents retrieves issues that depend on the given issue
// - Alias for GetDependencies with direction "up"
GetDependents(ctx context.Context, issueID string) ([]*types.Dependency, error)

// DetectCycles checks if adding a dependency would create a cycle
// - Uses recursive CTE with depth limit (100)
// - Only checks blocking dependency types
// - Returns true if cycle would be created
DetectCycles(ctx context.Context, fromID, toID string) (bool, error)

// GetDependencyTree builds a tree structure of dependencies
// - maxDepth limits recursion (default 10)
// - Returns nested structure suitable for tree rendering
GetDependencyTree(ctx context.Context, rootID string, maxDepth int) (*types.DependencyNode, error)

// GetAllDependencies retrieves all dependencies in the database
// - Used for export and cycle detection
GetAllDependencies(ctx context.Context) ([]*types.Dependency, error)
```

### 4.5 Label Operations

```go
// AddLabel adds a label to an issue
// - Creates entry in labels table
// - Creates "label_added" event
// - Marks issue as dirty
// - Idempotent: no error if label already exists
AddLabel(ctx context.Context, issueID, label string) error

// RemoveLabel removes a label from an issue
// - Removes entry from labels table
// - Creates "label_removed" event
// - Marks issue as dirty
// - No error if label didn't exist
RemoveLabel(ctx context.Context, issueID, label string) error

// GetLabels retrieves all labels for an issue
// - Returns slice of label strings
GetLabels(ctx context.Context, issueID string) ([]string, error)

// GetLabelsForIssues retrieves labels for multiple issues efficiently
// - Returns map[issueID][]label
// - Single query instead of N queries
GetLabelsForIssues(ctx context.Context, issueIDs []string) (map[string][]string, error)

// GetAllLabels retrieves all unique labels in the database
// - Returns slice of unique label strings
// - Sorted alphabetically
GetAllLabels(ctx context.Context) ([]string, error)

// GetIssuesByLabel retrieves all issues with a specific label
// - Returns slice of issues
// - Respects tombstone exclusion by default
GetIssuesByLabel(ctx context.Context, label string) ([]*types.Issue, error)
```

### 4.6 Comment Operations

```go
// AddComment adds a comment to an issue
// - Assigns auto-increment ID
// - Sets CreatedAt to current time
// - Creates "commented" event
// - Marks issue as dirty
// - Returns created comment with ID
AddComment(ctx context.Context, comment *types.Comment) (*types.Comment, error)

// GetComments retrieves all comments for an issue
// - Ordered by created_at ascending
// - Returns empty slice if no comments
GetComments(ctx context.Context, issueID string) ([]*types.Comment, error)

// GetCommentsForIssues retrieves comments for multiple issues
// - Returns map[issueID][]*Comment
// - Single query instead of N queries
GetCommentsForIssues(ctx context.Context, issueIDs []string) (map[string][]*types.Comment, error)

// DeleteComment removes a comment
// - Hard delete (comments don't have tombstones)
// - No event created (comments are metadata)
DeleteComment(ctx context.Context, commentID int64) error
```

### 4.7 Event Operations (Audit Trail)

```go
// CreateEvent records an event in the audit trail
// - Sets CreatedAt to current time
// - Returns created event with ID
CreateEvent(ctx context.Context, event *types.Event) (*types.Event, error)

// GetEvents retrieves events for an issue
// - Ordered by created_at ascending
// - Returns full audit history
GetEvents(ctx context.Context, issueID string) ([]*types.Event, error)

// GetEventsAfter retrieves events after a timestamp
// - Used for incremental sync
// - Returns events for all issues
GetEventsAfter(ctx context.Context, after time.Time) ([]*types.Event, error)

// GetRecentEvents retrieves most recent N events
// - Across all issues
// - Ordered by created_at descending
GetRecentEvents(ctx context.Context, limit int) ([]*types.Event, error)
```

### 4.8 Statistics Operations

```go
// GetStatistics computes project statistics
// - Counts by status, type, priority, assignee
// - Calculates average lead time (create -> close)
// - Returns Statistics struct
GetStatistics(ctx context.Context) (*types.Statistics, error)

// GetEpicsEligibleForClosure returns epics whose children are all closed
// - Epic status is open or in_progress
// - All child issues (parent-child deps) are closed or tombstone
GetEpicsEligibleForClosure(ctx context.Context) ([]*types.Issue, error)
```

### 4.9 Dirty Tracking Operations

```go
// MarkDirty marks an issue as needing export
// - Inserts into dirty_issues table
// - Idempotent: no error if already marked
MarkDirty(ctx context.Context, issueID string) error

// GetDirtyIssues retrieves all issues marked dirty
// - Returns issue IDs only (not full issues)
GetDirtyIssues(ctx context.Context) ([]string, error)

// ClearDirtyIssues clears all dirty flags
// - Called after successful export
ClearDirtyIssues(ctx context.Context) error

// ClearDirtyIssuesByID clears dirty flags for specific issues
// - Called after incremental export
ClearDirtyIssuesByID(ctx context.Context, ids []string) error

// HasDirtyIssues returns true if any issues are dirty
// - More efficient than GetDirtyIssues when only checking existence
HasDirtyIssues(ctx context.Context) (bool, error)
```

### 4.10 Export Hash Operations

```go
// SetExportHash records the content hash at export time
// - Used to detect external changes to JSONL
SetExportHash(ctx context.Context, issueID, contentHash string) error

// GetExportHash retrieves the last exported content hash
// - Returns empty string if never exported
GetExportHash(ctx context.Context, issueID string) (string, error)

// GetExportHashes retrieves export hashes for multiple issues
// - Returns map[issueID]hash
GetExportHashes(ctx context.Context, issueIDs []string) (map[string]string, error)

// ClearExportHashes removes all export hash records
// - Called when full re-export is needed
ClearExportHashes(ctx context.Context) error
```

### 4.11 Configuration Operations

```go
// GetConfig retrieves a configuration value
// - Returns empty string if not set
GetConfig(ctx context.Context, key string) (string, error)

// SetConfig sets a configuration value
// - Overwrites existing value if present
SetConfig(ctx context.Context, key, value string) error

// GetAllConfig retrieves all configuration key-value pairs
// - Returns map[key]value
GetAllConfig(ctx context.Context) (map[string]string, error)

// DeleteConfig removes a configuration key
// - No error if key didn't exist
DeleteConfig(ctx context.Context, key string) error
```

### 4.12 Metadata Operations

```go
// GetMetadata retrieves internal metadata
// - Used for sync state, import tracking, etc.
GetMetadata(ctx context.Context, key string) (string, error)

// SetMetadata sets internal metadata
SetMetadata(ctx context.Context, key, value string) error

// GetAllMetadata retrieves all metadata
GetAllMetadata(ctx context.Context) (map[string]string, error)
```

### 4.13 Transaction Support

```go
// RunInTransaction executes a function within a transaction
// - Uses BEGIN IMMEDIATE for write operations
// - Automatically commits on success, rolls back on error
// - Supports nested calls (inner calls are no-ops)
RunInTransaction(ctx context.Context, fn func(ctx context.Context) error) error

// BeginTx starts a new transaction manually
// - Returns transaction handle
// - Caller must call Commit() or Rollback()
BeginTx(ctx context.Context) (*sql.Tx, error)
```

### 4.14 Utility Operations

```go
// Close closes the storage connection
// - Flushes any pending operations
// - Releases database lock
Close() error

// Ping verifies database connectivity
// - Returns error if database is unreachable
Ping(ctx context.Context) error

// GetVersion returns the storage backend version
// - Includes migration version
GetVersion(ctx context.Context) (string, error)

// RunMigrations applies pending database migrations
// - Idempotent: already-applied migrations are skipped
// - Returns number of migrations applied
RunMigrations(ctx context.Context) (int, error)
```

---

## 5. SQLite Storage Implementation

### 5.1 Database Schema

#### Issues Table (Core)

```sql
CREATE TABLE IF NOT EXISTS issues (
    id TEXT PRIMARY KEY,
    content_hash TEXT,
    title TEXT NOT NULL CHECK(length(title) <= 500),
    description TEXT NOT NULL DEFAULT '',
    design TEXT NOT NULL DEFAULT '',
    acceptance_criteria TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'open',
    priority INTEGER NOT NULL DEFAULT 2 CHECK(priority >= 0 AND priority <= 4),
    issue_type TEXT NOT NULL DEFAULT 'task',
    assignee TEXT,
    owner TEXT DEFAULT '',
    estimated_minutes INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT DEFAULT '',
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at DATETIME,
    close_reason TEXT DEFAULT '',
    closed_by_session TEXT DEFAULT '',
    external_ref TEXT,
    due_at DATETIME,
    defer_until DATETIME,
    -- Compaction
    compaction_level INTEGER DEFAULT 0,
    compacted_at DATETIME,
    compacted_at_commit TEXT,
    original_size INTEGER,
    -- Tombstone
    deleted_at DATETIME,
    deleted_by TEXT DEFAULT '',
    delete_reason TEXT DEFAULT '',
    original_type TEXT DEFAULT '',
    -- Messaging
    sender TEXT DEFAULT '',
    ephemeral INTEGER DEFAULT 0,
    -- Context
    pinned INTEGER DEFAULT 0,
    is_template INTEGER DEFAULT 0,
    -- Federation
    source_system TEXT DEFAULT '',

    -- === CONSTRAINTS ===

    -- Closed-at invariant: closed issues MUST have closed_at timestamp
    CHECK (
        (status = 'closed' AND closed_at IS NOT NULL) OR
        (status = 'tombstone') OR
        (status NOT IN ('closed', 'tombstone') AND closed_at IS NULL)
    )
);

-- === INDEXES ===

-- Primary access patterns
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
CREATE INDEX IF NOT EXISTS idx_issues_priority ON issues(priority);
CREATE INDEX IF NOT EXISTS idx_issues_issue_type ON issues(issue_type);
CREATE INDEX IF NOT EXISTS idx_issues_assignee ON issues(assignee) WHERE assignee IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_issues_created_at ON issues(created_at);
CREATE INDEX IF NOT EXISTS idx_issues_updated_at ON issues(updated_at);

-- Export/sync patterns
CREATE INDEX IF NOT EXISTS idx_issues_content_hash ON issues(content_hash);
CREATE INDEX IF NOT EXISTS idx_issues_external_ref ON issues(external_ref) WHERE external_ref IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_issues_external_ref_unique ON issues(external_ref) WHERE external_ref IS NOT NULL;

-- Special states
CREATE INDEX IF NOT EXISTS idx_issues_ephemeral ON issues(ephemeral) WHERE ephemeral = 1;
CREATE INDEX IF NOT EXISTS idx_issues_pinned ON issues(pinned) WHERE pinned = 1;
CREATE INDEX IF NOT EXISTS idx_issues_tombstone ON issues(status) WHERE status = 'tombstone';

-- Time-based
CREATE INDEX IF NOT EXISTS idx_issues_due_at ON issues(due_at) WHERE due_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_issues_defer_until ON issues(defer_until) WHERE defer_until IS NOT NULL;

-- Ready work composite index (most important for performance)
CREATE INDEX IF NOT EXISTS idx_issues_ready
    ON issues(status, priority, created_at)
    WHERE status IN ('open', 'in_progress')
    AND ephemeral = 0
    AND pinned = 0;
```

#### Dependencies Table

```sql
CREATE TABLE IF NOT EXISTS dependencies (
    issue_id TEXT NOT NULL,
    depends_on_id TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'blocks',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT NOT NULL DEFAULT '',
    metadata TEXT DEFAULT '{}',
    thread_id TEXT DEFAULT '',

    PRIMARY KEY (issue_id, depends_on_id),
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
    -- Note: depends_on_id FK intentionally removed to allow external issue references
);

CREATE INDEX IF NOT EXISTS idx_dependencies_issue ON dependencies(issue_id);
CREATE INDEX IF NOT EXISTS idx_dependencies_depends_on ON dependencies(depends_on_id);
CREATE INDEX IF NOT EXISTS idx_dependencies_type ON dependencies(type);
CREATE INDEX IF NOT EXISTS idx_dependencies_depends_on_type ON dependencies(depends_on_id, type);
CREATE INDEX IF NOT EXISTS idx_dependencies_thread ON dependencies(thread_id) WHERE thread_id != '';

-- Composite for blocking lookups
CREATE INDEX IF NOT EXISTS idx_dependencies_blocking
    ON dependencies(depends_on_id, issue_id)
    WHERE type IN ('blocks', 'parent-child', 'conditional-blocks', 'waits-for');
```

#### Labels Table

```sql
CREATE TABLE IF NOT EXISTS labels (
    issue_id TEXT NOT NULL,
    label TEXT NOT NULL,
    PRIMARY KEY (issue_id, label),
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_labels_label ON labels(label);
CREATE INDEX IF NOT EXISTS idx_labels_issue ON labels(issue_id);
```

#### Comments Table

```sql
CREATE TABLE IF NOT EXISTS comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id TEXT NOT NULL,
    author TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_comments_issue ON comments(issue_id);
CREATE INDEX IF NOT EXISTS idx_comments_created_at ON comments(created_at);
```

#### Events Table (Audit Trail)

```sql
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL DEFAULT '',
    old_value TEXT,
    new_value TEXT,
    comment TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_events_issue ON events(issue_id);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at);
CREATE INDEX IF NOT EXISTS idx_events_actor ON events(actor) WHERE actor != '';
```

#### Config Table

```sql
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

#### Metadata Table

```sql
CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

#### Dirty Issues Table (Export Tracking)

```sql
CREATE TABLE IF NOT EXISTS dirty_issues (
    issue_id TEXT PRIMARY KEY,
    marked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_dirty_issues_marked_at ON dirty_issues(marked_at);
```

#### Export Hashes Table (Deduplication)

```sql
CREATE TABLE IF NOT EXISTS export_hashes (
    issue_id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    exported_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);
```

#### Blocked Issues Cache Table

```sql
-- Materialized view of blocked issues for performance
-- Rebuilt on dependency or status changes
CREATE TABLE IF NOT EXISTS blocked_issues_cache (
    issue_id TEXT PRIMARY KEY,
    blocked_by TEXT NOT NULL,  -- JSON array of blocking issue IDs
    blocked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_blocked_cache_blocked_at ON blocked_issues_cache(blocked_at);
```

#### Child Counters Table (Hierarchical IDs)

```sql
-- Tracks next child number for dotted IDs (bd-abc.1, bd-abc.2, etc.)
CREATE TABLE IF NOT EXISTS child_counters (
    parent_id TEXT PRIMARY KEY,
    last_child INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (parent_id) REFERENCES issues(id) ON DELETE CASCADE
);
```

### 5.2 SQLite Pragmas and Configuration

```sql
-- === Connection-Level Pragmas (set on every connection) ===

-- Enable foreign key enforcement
PRAGMA foreign_keys = ON;

-- Set busy timeout to 30 seconds (30000ms)
-- Prevents "database is locked" errors during concurrent access
PRAGMA busy_timeout = 30000;

-- Use Write-Ahead Logging for better concurrency
-- Exception: Use DELETE mode for:
--   - WSL2 with Windows filesystem (/mnt/c/, etc.) - WAL doesn't work across filesystems
--   - In-memory databases (:memory:) - WAL requires file
PRAGMA journal_mode = WAL;

-- === Performance Pragmas ===

-- Larger cache for better read performance (64MB)
PRAGMA cache_size = -65536;

-- Synchronous mode: NORMAL balances safety and speed
-- FULL is safer but slower, OFF is dangerous
PRAGMA synchronous = NORMAL;

-- Store temp tables in memory
PRAGMA temp_store = MEMORY;

-- Enable memory-mapped I/O for reads (256MB)
PRAGMA mmap_size = 268435456;
```

**Connection Pool Settings:**

```go
// File-based databases
MaxOpenConns = runtime.NumCPU() + 1  // e.g., 9 on 8-core
MaxIdleConns = 2
ConnMaxLifetime = 0  // No limit
ConnMaxIdleTime = 5 * time.Minute

// In-memory databases (must use single connection)
MaxOpenConns = 1
MaxIdleConns = 1
```

### 5.3 Transaction Handling

**BEGIN IMMEDIATE Pattern:**

```go
// For write operations, use BEGIN IMMEDIATE to acquire lock early
// This prevents deadlocks when multiple writers compete

func (s *Store) beginImmediateWithRetry(ctx context.Context, maxRetries int) (*sql.Tx, error) {
    baseDelay := 10 * time.Millisecond

    for attempt := 0; attempt < maxRetries; attempt++ {
        tx, err := s.db.BeginTx(ctx, nil)
        if err != nil {
            return nil, err
        }

        _, err = tx.ExecContext(ctx, "BEGIN IMMEDIATE")
        if err == nil {
            return tx, nil
        }

        tx.Rollback()

        // Check if error is SQLITE_BUSY (database locked)
        if !isBusyError(err) {
            return nil, err
        }

        // Exponential backoff: 10ms, 20ms, 40ms, 80ms, ...
        delay := baseDelay * (1 << attempt)
        if delay > 5*time.Second {
            delay = 5 * time.Second
        }

        select {
        case <-ctx.Done():
            return nil, ctx.Err()
        case <-time.After(delay):
            continue
        }
    }

    return nil, fmt.Errorf("failed to acquire lock after %d retries", maxRetries)
}
```

**Transaction Wrapper:**

```go
func (s *Store) RunInTransaction(ctx context.Context, fn func(context.Context) error) error {
    tx, err := s.beginImmediateWithRetry(ctx, 10)
    if err != nil {
        return err
    }

    defer func() {
        if p := recover(); p != nil {
            tx.Rollback()
            panic(p)
        }
    }()

    // Create context with transaction
    txCtx := context.WithValue(ctx, txKey, tx)

    if err := fn(txCtx); err != nil {
        tx.Rollback()
        return err
    }

    return tx.Commit()
}
```

### 5.4 Migration System

**Migration Format:**

Each migration is an idempotent SQL file. Migrations are tracked in a `schema_migrations` table:

```sql
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

**Migration List (40 migrations):**

| # | Name | Description |
|---|------|-------------|
| 001 | dirty_issues_table | Create dirty_issues table for export tracking |
| 002 | external_ref_column | Add external_ref column to issues |
| 003 | composite_indexes | Add composite indexes for common queries |
| 004 | closed_at_constraint | Add CHECK constraint for closed_at invariant |
| 005 | compaction_columns | Add compaction_level, compacted_at columns |
| 006 | snapshots_table | Create snapshots table for compaction |
| 007 | compaction_config | Add compaction config values |
| 008 | compacted_at_commit_column | Add compacted_at_commit column |
| 009 | export_hashes_table | Create export_hashes table for dedup |
| 010 | content_hash_column | Add content_hash column to issues |
| 011 | external_ref_unique | Add unique constraint on external_ref |
| 012 | source_repo_column | Add source_repo column for multi-repo |
| 013 | repo_mtimes_table | Create repo_mtimes table |
| 014 | child_counters_table | Create child_counters for hierarchical IDs |
| 015 | blocked_issues_cache | Create blocked_issues_cache table |
| 016 | orphan_detection | Add orphan handling support |
| 017 | close_reason_column | Add close_reason column |
| 018 | tombstone_columns | Add deleted_at, deleted_by, delete_reason, original_type |
| 019 | messaging_fields | Add sender, ephemeral columns |
| 020 | edge_consolidation | Add metadata, thread_id to dependencies |
| 021 | migrate_edge_fields | Migrate legacy edge data to new format |
| 022 | drop_edge_columns | Remove deprecated edge columns |
| 023 | pinned_column | Add pinned column |
| 024 | is_template_column | Add is_template column |
| 025 | remove_depends_on_fk | Remove FK on depends_on_id for external refs |
| 026 | additional_indexes | Add performance indexes |
| 027 | gate_columns | Add await_type, await_id, timeout, waiters, holder (Gastown) |
| 028 | tombstone_closed_at | Fix tombstone/closed_at handling |
| 029 | created_by_column | Add created_by column |
| 030 | agent_fields | Add hook_bead, role_bead, agent_state, etc. (Gastown) |
| 031 | mol_type_column | Add mol_type column (Gastown) |
| 032 | hooked_status_migration | Add hooked status support (Gastown) |
| 033 | event_fields | Add event_kind, actor, target, payload |
| 034 | closed_by_session_column | Add closed_by_session column |
| 035 | due_defer_columns | Add due_at, defer_until columns |
| 036 | owner_column | Add owner column |
| 037 | crystallizes_column | Add crystallizes column (HOP) |
| 038 | work_type_column | Add work_type column (Gastown) |
| 039 | source_system_column | Add source_system column |
| 040 | quality_score_column | Add quality_score column (HOP) |

**Migration Execution:**

```go
func (s *Store) RunMigrations(ctx context.Context) (int, error) {
    // Get current version
    var currentVersion int
    row := s.db.QueryRowContext(ctx, "SELECT COALESCE(MAX(version), 0) FROM schema_migrations")
    row.Scan(&currentVersion)

    applied := 0
    for version, sql := range migrations {
        if version <= currentVersion {
            continue
        }

        // Run migration in transaction
        tx, _ := s.db.BeginTx(ctx, nil)
        tx.ExecContext(ctx, sql)
        tx.ExecContext(ctx, "INSERT INTO schema_migrations (version) VALUES (?)", version)
        tx.Commit()

        applied++
    }

    return applied, nil
}
```

---

## 6. CLI Commands Specification

### 6.1 Global Flags

All commands support these global flags:

```
--db <path>           Database path (auto-discovers .beads/*.db if not specified)
--actor <name>        Actor name for audit trail (default: git user or $USER)
--json                Output in JSON format (machine-readable)
--no-daemon           Force direct storage mode, bypass daemon
--no-auto-flush       Skip automatic JSONL export after changes
--no-auto-import      Skip automatic JSONL import before queries
--verbose, -v         Enable verbose debug output
--quiet, -q           Suppress non-essential output
--lock-timeout <ms>   SQLite busy timeout in milliseconds (default: 30000)
--help, -h            Show help for command
```

### 6.2 `init` Command

**Purpose:** Initialize a beads workspace in the current directory.

```bash
bd init [flags]

Flags:
  --prefix <string>    Issue ID prefix (default: "bd")
  --force              Overwrite existing .beads/ directory
```

**Behavior:**
1. Creates `.beads/` directory
2. Creates `.beads/beads.db` SQLite database
3. Runs all migrations
4. Sets `issue_prefix` config if --prefix specified
5. Creates `.beads/.gitignore` with:
   ```
   beads.db
   beads.db-wal
   beads.db-shm
   bd.sock
   daemon.log
   export_hashes.db
   sync_base.jsonl
   ```

**Output:**
- Text: `Initialized beads workspace in .beads/`
- JSON: `{"status": "initialized", "path": ".beads/", "prefix": "bd"}`

### 6.3 `create` Command

**Purpose:** Create a new issue.

```bash
bd create <title> [flags]

Arguments:
  title                Issue title (required, max 500 chars)

Flags:
  --type, -t <type>           Issue type (default: task)
                              Values: bug, feature, task, epic, chore, docs, question
  --priority, -p <int>        Priority 0-4 or P0-P4 (default: 2)
  --description, -d <text>    Description text (multi-line OK)
  --design <text>             Design specification
  --acceptance <text>         Acceptance criteria
  --notes <text>              Additional notes
  --assignee, -a <name>       Assign to person
  --owner <email>             Owner email (default: git author)
  --labels, -l <labels>       Comma-separated labels
  --parent <id>               Parent issue ID (creates parent-child dep)
  --deps <deps>               Dependencies (format: type:id,type:id)
                              Examples: blocks:bd-abc, discovered-from:bd-def
  --estimate, -e <minutes>    Time estimate in minutes
  --due <datetime>            Due date (RFC3339 or relative: "tomorrow", "2024-12-31")
  --defer <datetime>          Defer until date
  --external-ref <ref>        External reference (e.g., "gh-123", "JIRA-456")
  --ephemeral                 Mark as ephemeral (not exported to JSONL)
  --dry-run                   Preview without creating
  --silent                    Output only issue ID (for scripting)
```

**Behavior:**
1. Validates title length (1-500 chars)
2. Validates priority range (0-4)
3. Generates hash-based ID
4. Creates issue in database
5. Adds dependencies if specified
6. Adds labels if specified
7. Marks as dirty for export
8. Creates "created" event

**Output:**
- Text: `Created bd-abc123: Issue title`
- Silent: `bd-abc123`
- JSON:
  ```json
  {
    "id": "bd-abc123",
    "title": "Issue title",
    "status": "open",
    "priority": 2,
    "issue_type": "task",
    "created_at": "2024-01-15T10:30:00Z"
  }
  ```

### 6.4 `update` Command

**Purpose:** Update an existing issue.

```bash
bd update <id> [flags]

Arguments:
  id                   Issue ID (full or prefix, or "." for last touched)

Flags:
  --title <text>              New title
  --description, -d <text>    New description
  --design <text>             New design spec
  --acceptance <text>         New acceptance criteria
  --notes <text>              New notes
  --status, -s <status>       New status (open, in_progress, blocked, deferred)
  --priority, -p <int>        New priority 0-4
  --type, -t <type>           New issue type
  --assignee, -a <name>       New assignee (use "" to clear)
  --owner <email>             New owner
  --estimate, -e <minutes>    New time estimate
  --due <datetime>            New due date (use "" to clear)
  --defer <datetime>          New defer date (use "" to clear)
  --external-ref <ref>        New external reference
  --add-label <label>         Add a label
  --remove-label <label>      Remove a label
  --pinned <bool>             Set pinned status
```

**Behavior:**
1. Resolves issue ID (exact match or prefix)
2. Validates new values
3. Updates specified fields only (others unchanged)
4. Updates UpdatedAt timestamp
5. Recomputes ContentHash
6. Creates event(s) for changed fields
7. Marks as dirty

**Output:**
- Text: `Updated bd-abc123`
- JSON: Full issue object with updated fields

### 6.5 `close` Command

**Purpose:** Close one or more issues.

```bash
bd close <id>... [flags]

Arguments:
  id                   Issue ID(s) to close (supports multiple)

Flags:
  --reason, -r <text>         Reason for closing
  --force                     Close even if blocked
  --suggest-next              After closing, show next ready issue
  --session <id>              Session ID (for attribution)
```

**Behavior:**
1. Resolves each issue ID
2. For each issue:
   - Checks if blocked (fails unless --force)
   - Sets status to "closed"
   - Sets ClosedAt to current time
   - Sets CloseReason if provided
   - Sets ClosedBySession if provided
   - Creates "closed" event
   - Marks as dirty
   - Refreshes blocked cache (may unblock dependents)
3. If --suggest-next, queries ready work and shows first result

**Output:**
- Text: `Closed bd-abc123: Issue title`
- JSON: Array of closed issue objects

### 6.6 `list` Command

**Purpose:** List issues matching criteria.

```bash
bd list [flags]

Flags:
  --status, -s <status>       Filter by status (comma-separated for multiple)
  --priority, -p <int>        Filter by priority
  --type, -t <type>           Filter by type
  --assignee, -a <name>       Filter by assignee
  --unassigned                Show only unassigned issues
  --label, -l <label>         Filter by label (AND if multiple -l flags)
  --label-any <labels>        Filter by any label (OR, comma-separated)
  --query, -q <text>          Full-text search in title/description
  --created-after <date>      Filter by creation date
  --created-before <date>     Filter by creation date
  --updated-after <date>      Filter by update date
  --overdue                   Show only overdue issues
  --deferred                  Show only deferred issues
  --pinned                    Show only pinned issues
  --include-tombstones        Include soft-deleted issues
  --sort <field>              Sort by field (priority, created_at, updated_at)
  --order <asc|desc>          Sort order (default: asc)
  --limit, -n <int>           Maximum results (default: 50)
  --offset <int>              Skip first N results
  --pretty                    Pretty tree format with Unicode
  --no-header                 Omit header in table output
```

**Output Formats:**

Text (default):
```
ID         PRI  TYPE     STATUS       ASSIGNEE  TITLE
bd-abc123  P1   feature  in_progress  alice     Add dark mode
bd-def456  P2   bug      open         bob       Fix login error
```

Pretty (--pretty):
```
○ bd-abc123 [P1] Add dark mode (feature) @alice
● bd-def456 [P2] Fix login error (bug) @bob
```

JSON:
```json
{
  "issues": [...],
  "total": 42,
  "limit": 50,
  "offset": 0
}
```

### 6.7 `show` Command

**Purpose:** Show detailed information about an issue.

```bash
bd show <id> [flags]

Arguments:
  id                   Issue ID (or "." for last touched)

Flags:
  --short              Show compact format
  --deps               Show dependency tree
  --comments           Show comments
  --events             Show event history
  --refs               Show what references this issue
  --no-color           Disable color output
```

**Output:**

Text (full):
```
bd-abc123: Add dark mode toggle
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Status:    in_progress          Priority: P1 (high)
Type:      feature              Assignee: alice
Created:   2024-01-10 10:00     Updated:  2024-01-15 14:30
Due:       2024-01-20

Description:
  Add a dark mode toggle to the application settings page.

Acceptance Criteria:
  - Toggle saves preference to localStorage
  - System preference detected on first visit
  - Smooth transition animation

Labels: ui, enhancement

Dependencies:
  └── blocks: bd-def456 (CSS refactor)

Blocking:
  └── bd-ghi789 depends on this
```

JSON: Full issue object with all nested data

### 6.8 `ready` Command

**Purpose:** Show issues ready to be worked on.

```bash
bd ready [flags]

Flags:
  --limit, -n <int>           Maximum results (default: 10)
  --assignee, -a <name>       Filter by assignee
  --unassigned                Show only unassigned
  --sort <policy>             Sort policy: hybrid, priority, oldest
                              hybrid: P0-P1 first, then oldest
  --label, -l <label>         Filter by label (AND)
  --label-any <labels>        Filter by labels (OR)
  --type, -t <type>           Filter by type
  --priority, -p <int>        Filter by priority
  --include-deferred          Include deferred issues
  --pretty                    Pretty tree format
```

**Behavior:**
1. Queries issues where:
   - Status is "open" or "in_progress"
   - NOT in blocked_issues_cache
   - NOT deferred (defer_until is NULL or in past)
   - NOT pinned
   - NOT ephemeral
2. Applies additional filters
3. Sorts by policy
4. Returns up to limit

**Sort Policies:**

- **hybrid** (default): P0-P1 issues first by creation date, then P2-P4 by creation date
- **priority**: By priority ascending, then creation date
- **oldest**: By creation date ascending only

**Output:**

Text:
```
Ready to work (3 issues):

○ bd-abc123 [P0] Critical security fix (bug)
○ bd-def456 [P1] Add user authentication (feature)
○ bd-ghi789 [P2] Update documentation (docs)
```

JSON:
```json
{
  "issues": [...],
  "count": 3
}
```

### 6.9 `blocked` Command

**Purpose:** Show blocked issues and what's blocking them.

```bash
bd blocked [flags]

Flags:
  --limit, -n <int>           Maximum results
  --verbose                   Show full blocking chain
```

**Output:**

Text:
```
Blocked issues (2):

● bd-abc123 [P1] Implement OAuth (feature)
  └── Blocked by: bd-xyz789 (open)

● bd-def456 [P2] Add payment flow (feature)
  └── Blocked by: bd-abc123 (in_progress)
```

JSON:
```json
{
  "blocked_issues": [
    {
      "issue": {...},
      "blocked_by": [
        {"id": "bd-xyz789", "status": "open", "title": "..."}
      ]
    }
  ],
  "count": 2
}
```

### 6.10 `dep` Command

**Purpose:** Manage issue dependencies.

```bash
bd dep <subcommand> [flags]

Subcommands:
  add       Add a dependency
  remove    Remove a dependency
  list      List dependencies
  tree      Show dependency tree
  cycles    Detect dependency cycles
```

**`dep add`:**
```bash
bd dep add <issue> <depends-on> [flags]

Arguments:
  issue                Issue that depends on another
  depends-on           Issue being depended on

Flags:
  --type, -t <type>    Dependency type (default: blocks)
                       Values: blocks, parent-child, related, discovered-from,
                               conditional-blocks, waits-for, duplicates, supersedes
  --metadata <json>    Additional metadata (JSON object)
```

Behavior:
1. Resolves both issue IDs
2. Validates dependency type
3. Checks for cycles (if blocking type)
4. Creates dependency record
5. Refreshes blocked cache
6. Creates "dependency_added" event

**`dep remove`:**
```bash
bd dep remove <issue> <depends-on>
```

**`dep list`:**
```bash
bd dep list <issue> [flags]

Flags:
  --direction <dir>    down: what this depends on
                       up: what depends on this
                       both: all (default)
```

**`dep tree`:**
```bash
bd dep tree <issue> [flags]

Flags:
  --max-depth, -d <int>   Maximum depth (default: 10)
  --format <format>       Output format: text, mermaid
```

Output (text):
```
bd-epic-1 [Epic: User Management]
├── bd-task-1 [P1] Design schema
│   ├── bd-task-2 [P2] Implement models
│   │   └── bd-task-3 [P2] Write tests
│   └── bd-task-4 [P2] API endpoints
└── bd-task-5 [P1] Documentation
```

Output (mermaid):
```mermaid
graph TD
    bd-epic-1["Epic: User Management"]
    bd-task-1["P1: Design schema"]
    bd-epic-1 --> bd-task-1
    ...
```

**`dep cycles`:**
```bash
bd dep cycles
```

Detects and reports any cycles in blocking dependencies.

### 6.11 `label` Command

**Purpose:** Manage labels.

```bash
bd label <subcommand> [args]

Subcommands:
  add <issue> <label>     Add label to issue
  remove <issue> <label>  Remove label from issue
  list [issue]            List labels (for issue or all unique labels)
```

### 6.12 `search` Command

**Purpose:** Full-text search across issues.

```bash
bd search <query> [flags]

Arguments:
  query                  Search query

Flags:
  --status, -s <status>  Filter by status
  --type, -t <type>      Filter by type
  --limit, -n <int>      Maximum results (default: 20)
```

**Behavior:**
- Searches title and description fields
- Uses SQLite FTS5 if available
- Falls back to LIKE with wildcards

### 6.13 `stats` Command

**Purpose:** Show project statistics.

```bash
bd stats [flags]

Flags:
  --by-type              Show breakdown by issue type
  --by-priority          Show breakdown by priority
  --by-assignee          Show breakdown by assignee
  --by-label             Show breakdown by label
```

**Output:**

```
Issue Statistics
================

Total:        142
Open:          45  (31.7%)
In Progress:   12  (8.5%)
Closed:        78  (54.9%)
Blocked:        5  (3.5%)
Deferred:       2  (1.4%)

Ready to work: 38
Avg lead time: 4.2 days

By Priority:
  P0:  3 issues
  P1: 15 issues
  P2: 67 issues
  P3: 42 issues
  P4: 15 issues
```

### 6.14 `sync` Command

**Purpose:** Synchronize database with JSONL and optionally git.

```bash
bd sync [flags]

Flags:
  --flush-only           Export to JSONL only (no git operations)
  --import-only          Import from JSONL only
  --dry-run              Show what would change without applying
  --no-pull              Skip git pull before import
  --no-push              Skip git push after export
  --status               Show sync status without making changes
  --message, -m <text>   Custom git commit message
```

**Behavior:**

1. **Import phase** (unless --flush-only):
   - Check if JSONL is newer than database
   - Parse JSONL file
   - Detect collisions (same ID, different content)
   - Merge changes into database

2. **Export phase** (unless --import-only):
   - Get all issues (including tombstones, excluding ephemeral)
   - Populate dependencies, labels, comments
   - Compute content hashes
   - Write to temp file atomically
   - Rename to issues.jsonl

3. **Git phase** (unless --flush-only or --no-push):
   - Stage .beads/issues.jsonl
   - Commit with message
   - Push to remote

### 6.15 `config` Command

**Purpose:** Manage configuration.

```bash
bd config <subcommand> [args]

Subcommands:
  get <key>              Get config value
  set <key> <value>      Set config value
  list                   List all config values
  delete <key>           Delete config key
```

---

## 7. JSONL Import/Export System

### 7.1 Export Error Policies

The export system supports four configurable error handling policies:

| Policy | Behavior | Use Case |
|--------|----------|----------|
| `strict` (default) | Fails immediately on any error, no partial exports | User-initiated exports |
| `best-effort` | Skips failures with stderr warnings, continues | Auto-export background operations |
| `partial` | Retries transient failures with exponential backoff (100ms→200ms→400ms), skips persistent failures | Resilient batch operations |
| `required-core` | Fails on core data (issues/deps), best-effort for enrichments (labels/comments) | Production sync |

**Configuration:**

| Key | Default | Description |
|-----|---------|-------------|
| `export.error_policy` | `"strict"` | Default policy for exports |
| `auto_export.error_policy` | `"best-effort"` | Override for background auto-export |
| `export.retry_attempts` | `3` | Max retry attempts for transient failures |
| `export.retry_backoff_ms` | `100` | Initial backoff (doubles each retry) |
| `export.skip_encoding_errors` | `false` | Skip issues that fail JSON encoding |
| `export.write_manifest` | `false` | Write `.manifest.json` with export metadata |

**Export Manifest Fields** (when enabled):

| Field | Type | Description |
|-------|------|-------------|
| `exported_count` | int | Total issues successfully exported |
| `failed_issues` | array | Issues that failed with reason and missing data |
| `partial_data` | array | Data types that had failures (e.g., `["labels"]`) |
| `warnings` | array | Non-fatal warnings encountered |
| `complete` | bool | True if export had no errors |
| `exported_at` | timestamp | RFC3339 export timestamp |
| `error_policy` | string | Policy used for this export |

### 7.2 File Format

**Location:** `.beads/issues.jsonl`

**Format:** One complete issue JSON object per line. No trailing commas. UTF-8 encoding.

```json
{"id":"bd-abc123","title":"Fix bug","status":"open","priority":1,"issue_type":"bug","created_at":"2024-01-15T10:00:00Z","updated_at":"2024-01-15T10:00:00Z"}
{"id":"bd-def456","title":"Add feature","status":"closed","priority":2,"issue_type":"feature","created_at":"2024-01-14T09:00:00Z","updated_at":"2024-01-15T11:00:00Z","closed_at":"2024-01-15T11:00:00Z"}
```

**Fields exported:**
- All Issue struct fields with `json` tags (except `json:"-"` fields)
- `labels` array (embedded)
- `dependencies` array (embedded)
- `comments` array (embedded)

**Fields NOT exported:**
- `content_hash` (computed, not serialized)
- `source_repo` (internal routing)
- `id_prefix` (internal routing)
- `prefix_override` (internal routing)

**Ephemeral issues:** Issues with `ephemeral: true` are NOT exported.

**Tombstones:** Issues with `status: "tombstone"` ARE exported (for sync).

### 7.3 Export Flow

```
┌──────────────────────┐
│  GetAllIssues()      │ ─── Includes tombstones, excludes ephemeral
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Populate relations  │ ─── Labels, dependencies, comments
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Compute hashes      │ ─── ContentHash for each issue
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Write temp file     │ ─── .beads/issues.jsonl.tmp
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Atomic rename       │ ─── mv tmp -> issues.jsonl
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Update metadata     │ ─── Set jsonl_content_hash, last_export_time
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Clear dirty flags   │ ─── ClearDirtyIssues()
└──────────────────────┘
```

**Atomic Write Requirements:**
1. Write to temp file (`.jsonl.tmp`) in same directory
2. Use 2MB buffered writer for performance
3. Disable HTML escaping in JSON encoder (preserve `<`, `>`, `&`)
4. Flush buffer, fsync file, then atomic rename to final path
5. On any error: close file, delete temp, return error

### 7.4 Import Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `dry_run` | bool | false | Preview changes without applying |
| `skip_update` | bool | false | Create-only mode (skip existing issues) |
| `strict` | bool | false | Fail on any error |
| `rename_on_import` | bool | false | Rename issues to match database prefix |
| `skip_prefix_validation` | bool | false | Allow any prefix (multi-repo mode) |
| `orphan_handling` | enum | `allow` | How to handle missing parent issues |
| `clear_duplicate_external_refs` | bool | false | Clear duplicate external_ref values |
| `protect_local_export_ids` | map | empty | Timestamp-aware protection (GH#865) |

**Orphan Handling Modes:**

| Mode | Behavior |
|------|----------|
| `strict` | Fail if parent issue not found |
| `resurrect` | Auto-resurrect parent from history |
| `skip` | Skip orphaned issues silently |
| `allow` | Import without parent validation (default) |

**Timestamp-Aware Protection (GH#865):** When `protect_local_export_ids` contains an issue ID with a timestamp, import will skip updating that issue if the incoming `updated_at` is older than the protection timestamp. This prevents import from overwriting recently-exported local changes.

### 7.5 Import Flow

```
┌──────────────────────┐
│  Check staleness     │ ─── Compare mtime + content hash using Lstat()
└──────────┬───────────┘
           │ (if newer)
           v
┌──────────────────────┐
│  Check git markers   │ ─── Detect merge conflicts (<<<<<<<)
└──────────┬───────────┘
           │ (if no conflicts)
           v
┌──────────────────────┐
│  Parse JSONL         │ ─── Stream with 2MB buffer
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Normalize issues    │ ─── Canonicalize refs, compute hashes
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Collision detection │ ─── Same ID, different content
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Begin transaction   │
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Upsert issues       │ ─── INSERT OR REPLACE
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Sync dependencies   │ ─── Delete old, insert new
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Sync labels         │ ─── Delete old, insert new
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Sync comments       │ ─── Delete old, insert new
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Commit transaction  │
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Refresh caches      │ ─── blocked_issues_cache
└──────────┬───────────┘
           │
           v
┌──────────────────────┐
│  Update metadata     │ ─── last_import_time, jsonl_file_hash
└──────────────────────┘
```

### 7.6 Staleness Detection

**Algorithm:** Database is considered stale (needs import) when JSONL file has changed since last import.

**Detection Steps:**
1. Read `last_import_time` from metadata (RFC3339Nano or RFC3339 format)
2. If no metadata: assume fresh (not stale)
3. Get JSONL file mtime using **Lstat()** (NOT Stat())
   - **Critical:** Lstat() returns symlink's own mtime, not target's
   - Required for NixOS and systems where JSONL may be symlinked
4. If `file_mtime > last_import_time`:
   - Compute SHA256 of file content
   - Compare against stored `jsonl_content_hash` (or legacy key `last_import_hash`)
   - Stale only if hashes differ (handles `touch` and unchanged git pulls)

**Metadata Keys:**
| Key | Description |
|-----|-------------|
| `last_import_time` | RFC3339 timestamp of last successful import |
| `jsonl_content_hash` | SHA256 of JSONL file after last import |
| `last_import_hash` | Legacy key (migration compatibility) |

### 7.7 Git Conflict Detection

Before parsing JSONL, scan for git merge conflict markers:
- `<<<<<<< ` (conflict start)
- `=======` (separator)
- `>>>>>>> ` (conflict end)

If any marker found: abort import with error instructing user to resolve conflicts or regenerate with `bd export --force`.

### 7.8 Issue Normalization

Before collision detection, normalize all incoming issues:

| Step | Action | Rationale |
|------|--------|-----------|
| Ephemeral detection | If ID contains `-wisp-`, set `ephemeral=true` | Prevent patrol/workflow instances from polluting `bd ready` |
| Hash recomputation | Recompute `content_hash` from fields | JSONL hashes may be stale or incorrect |
| External ref canonicalization | Canonicalize Linear refs (e.g., slug → ID) | Prevent duplicate issues from different ref formats |

### 7.9 Collision Detection (4-Phase Algorithm)

For each incoming issue, match against database in priority order:

| Phase | Match By | Outcome |
|-------|----------|---------|
| 0 | `external_ref` | Update existing (cross-system sync) |
| 1a | `content_hash` + same ID | Exact match (skip, idempotent) |
| 1b | `content_hash` + different ID | Rename detection |
| 2 | ID only | Update (same ID, different content) |
| 3 | No match | Create new issue |

**Special Cases:**
- **Tombstone protection:** If existing issue is tombstone status, skip incoming (never resurrect deleted issues)
- **Timestamp-aware protection (GH#865):** If issue ID is in `protect_local_export_ids` with timestamp newer than incoming `updated_at`, skip update
- **Cross-prefix duplicates:** Same content hash but different prefix → skip incoming, keep existing

**Collision Resolution (Last-Write-Wins by UpdatedAt):**

| Condition | Action |
|-----------|--------|
| `remote.UpdatedAt > local.UpdatedAt` | Take remote version (remote wins) |
| `remote.UpdatedAt <= local.UpdatedAt` | Skip update (local wins or tie) |

**Tombstone Protection (CRITICAL):**

| Condition | Action | Rationale |
|-----------|--------|-----------|
| Existing issue has `status = tombstone` | Skip incoming issue | Tombstones are permanent; never resurrect deleted issues |

**Duplicate External Reference Detection:**

Validates that no two issues share the same `external_ref` (required for cross-system sync integrity).

| Mode | Behavior |
|------|----------|
| Strict (`clearDuplicates=false`) | Return error listing all duplicate refs |
| Auto-fix (`clearDuplicates=true`) | Keep first occurrence, clear `external_ref` on subsequent |

Detection Algorithm:
1. Build map of `external_ref → [issue_ids]`
2. For each ref with multiple IDs: flag as duplicate
3. In auto-fix mode: set `external_ref = NULL` on all but first issue

### 7.6 Dirty Tracking

Issues are marked dirty on any modification:

| Operation | Mark Dirty |
|-----------|------------|
| CreateIssue | Yes |
| UpdateIssue | Yes |
| CloseIssue | Yes |
| ReopenIssue | Yes |
| DeleteIssue | Yes |
| RestoreIssue | Yes |
| AddDependency | Both issues |
| RemoveDependency | Both issues |
| AddLabel | Yes |
| RemoveLabel | Yes |
| AddComment | Yes |

**Dirty Tracking Operations:**

| Operation | Interface | Behavior |
|-----------|-----------|----------|
| `MarkIssueDirty(issueID)` | Single issue | Upsert into `dirty_issues` with current timestamp |
| `MarkIssuesDirty(issueIDs)` | Batch | Upsert all in single transaction |
| `GetDirtyIssues()` | Query | Return issue IDs ordered by `marked_at` ASC (FIFO) |
| `GetDirtyIssueCount()` | Query | Return count for monitoring |
| `ClearDirtyIssuesByID(issueIDs)` | Cleanup | Delete specific IDs (only those actually exported) |

**Export Hash Operations:**

The `export_hashes` table enables incremental export by tracking content hashes of previously exported issues.

| Operation | Interface | Behavior |
|-----------|-----------|----------|
| `GetExportHash(issueID)` | Query | Return stored content hash, empty string if none |
| `SetExportHash(issueID, hash)` | Upsert | Store hash with `exported_at` timestamp |
| `ClearAllExportHashes()` | Bulk delete | Invalidate all hashes (required before import) |

**JSONL File Hash Tracking:**

| Operation | Interface | Behavior |
|-----------|-----------|----------|
| `GetJSONLFileHash()` | Query | Get `jsonl_file_hash` from metadata table |
| `SetJSONLFileHash(hash)` | Upsert | Store hash in metadata (bd-160) |

### 7.10 Incremental Export Algorithm

1. **Get dirty issues:** Query `dirty_issues` table for issue IDs (FIFO order)
2. **Filter by hash:** For each dirty ID:
   - Load issue from database (skip if deleted)
   - Compute current `content_hash`
   - Compare against stored `export_hash`
   - Only include if hashes differ (content actually changed)
3. **Atomic write:** Write filtered issues to JSONL (see Section 7.3)
4. **Update hashes:** For each exported issue, call `SetExportHash(id, hash)`
5. **Clear dirty flags:** Call `ClearDirtyIssuesByID()` with exported IDs only

**Critical:** Only clear dirty flags for issues that were actually exported. This prevents race conditions where new changes arrive between steps 1 and 5.

### 7.11 Race Condition Prevention (GH#607)

**Problem:** Database reconnection logic could close connections mid-query.

**Solution:** All storage operations acquire a read lock (`reconnectMu.RLock()`) before executing database operations. The reconnect function acquires a write lock, ensuring mutual exclusion.

| Lock Type | Holder | Purpose |
|-----------|--------|---------|
| Read lock | All query/exec operations | Prevent reconnect during operation |
| Write lock | Reconnect function | Ensure no operations in progress |

### 7.12 Export Hash Invalidation on Import (CRITICAL)

Before importing from JSONL, **all export hashes must be cleared** because:
- Imported issues may have different content than exported versions
- Stored hashes would cause false "no change" detection
- Incremental export would skip issues that need re-export

**Invariant:** `ClearAllExportHashes()` must be called at the start of any import operation.

### 7.13 3-Way Merge (Full Sync)

For complex sync scenarios with concurrent local and remote changes.

**Data Sources:**

| Source | Location | Description |
|--------|----------|-------------|
| Base | `.beads/sync_base.jsonl` | Snapshot at last successful sync |
| Local | Database | Current local state |
| Remote | JSONL after git pull | Incoming remote state |

**Merge Decision Table:**

| Present In | Action | Rationale |
|------------|--------|-----------|
| Remote only | Import | New issue from remote |
| Local only | Keep | New local issue |
| Base only | Delete | Deleted on remote (tombstone) |
| Local + Remote (identical) | No change | Already in sync |
| Local + Remote (differ) + Base | See below | 3-way comparison needed |
| Local + Remote (differ) - Base | LWW by `UpdatedAt` | No base to determine origin |

**3-Way Comparison (when Base exists):**

| Condition | Action | Rationale |
|-----------|--------|-----------|
| `Local == Base` | Take Remote | Local unchanged, remote modified |
| `Remote == Base` | Keep Local | Remote unchanged, local modified |
| All three differ | LWW by `UpdatedAt` | Both modified, resolve by timestamp |

---

## 8. Ready/Blocked Logic and Dependency Graph

### 8.1 Ready Work Definition

An issue is "ready to work" if ALL of the following are true:

1. **Status is active:** `status IN ('open', 'in_progress')`
2. **Not blocked:** `id NOT IN blocked_issues_cache`
3. **Not deferred:** `defer_until IS NULL OR defer_until <= CURRENT_TIMESTAMP`
4. **Not pinned:** `pinned = 0`
5. **Not ephemeral:** `ephemeral = 0`

### 8.2 Blocking Calculation

An issue is blocked if ANY of the following conditions are met:

**Direct Blocking:**
- Has a `blocks` dependency on an issue that is NOT closed/tombstone
- Has a `conditional-blocks` dependency on an issue that hasn't failed

**Inherited Blocking (Parent-Child):**
- Has a `parent-child` dependency on an issue that is blocked (transitive)
- Parent is not closed → children are blocked

**Waits-For Blocking:**
- Has a `waits-for` dependency with pending (non-closed) children

### 8.3 Blocked Issues Cache

The `blocked_issues_cache` table is a materialized view rebuilt on:
- Dependency added/removed
- Issue status changed (especially to/from closed)
- Manual refresh request

**Rebuild Algorithm:**

The cache rebuild is a 2-phase process using a recursive CTE:

**Phase 1: Direct Blocking Detection**

Find issues blocked directly by each dependency type:

| Dependency Type | Blocking Condition | Join/Lookup |
|-----------------|-------------------|-------------|
| `blocks` | Blocker has status: `open`, `in_progress`, `blocked`, `deferred`, `hooked` | Join on `depends_on_id` → `issues.id` |
| `conditional-blocks` | Blocker NOT closed, OR closed without failure keyword in `close_reason` | Same join + keyword check |
| `waits-for` (all-children) | ANY child of spawner has non-closed status | Subquery on `parent-child` deps |
| `waits-for` (any-children) | NO children of spawner are closed | Subquery with NOT EXISTS |

**Phase 2: Transitive Propagation**

Using recursive CTE, propagate blockage through `parent-child` relationships:
- Base case: All directly blocked issues (from Phase 1)
- Recursive case: For each blocked issue, find all issues with `parent-child` dependency pointing to it
- Depth limit: 50 levels to prevent infinite recursion
- Final output: DISTINCT union of all blocked issues

**Blocking Semantics by Type:**

| Type | Semantics | Unblocked When |
|------|-----------|----------------|
| `blocks` | B blocked until A completes | A's status is `closed` or `tombstone` |
| `conditional-blocks` | B runs only if A fails | A closed with failure close_reason |
| `waits-for` (all-children) | B waits for spawner's children | ALL children of spawner closed |
| `waits-for` (any-children) | B waits for any child | ANY child of spawner closed |
| `parent-child` | Children inherit parent blocking | Parent unblocked (transitive) |

**Failure Close Reason Keywords:**
- `failed`, `rejected`, `wontfix`, `won't fix`
- `cancelled`, `canceled`, `abandoned`
- `blocked`, `error`, `timeout`, `aborted`

**Cache Invalidation Triggers:**
- `blocks`, `conditional-blocks`, `waits-for`, or `parent-child` dependency added/removed
- Any issue's status changes (affects whether it blocks others)
- Issue closed (closed issues don't block others; conditional-blocks checks close_reason)

**NOT Invalidation Triggers:**
- `related` dependencies added/removed (informational only)
- `discovered-from` dependencies (provenance tracking only)

**Performance Characteristics:**
- Without cache: O(n²) for n issues (recursive traversal per query)
- With cache: O(1) lookup per issue
- Cache rebuild: O(n × d) where d is average dependency depth
- Typical speedup: 25x for ready work queries

### 8.4 Ready Work Query

**Filter Conditions (all must pass):**

| Condition | Filter |
|-----------|--------|
| Active status | `status IN ('open', 'in_progress')` |
| Not blocked | `id NOT IN blocked_issues_cache` |
| Not deferred | `defer_until IS NULL` OR `defer_until <= now` |
| Not pinned | `pinned = 0` |
| Not ephemeral | `ephemeral = 0` |

**Sort Order:**
1. **Priority tier:** P0-P1 (critical/high) before P2-P4
2. **Within tier:** Lower priority number first
3. **Within priority:** Older `created_at` first (FIFO)

**Result:** Limited to requested count (default varies by command)

### 8.5 Cycle Detection

Cycles are only checked for blocking dependency types.

**Algorithm:**
1. Start from source issue
2. Recursively traverse all blocking dependencies (`blocks`, `parent-child`, `conditional-blocks`, `waits-for`)
3. Track visited path to detect cycles
4. Depth limit: 100 levels

**Cycle-Relevant Dependency Types:**

| Type | Check for Cycles |
|------|-----------------|
| `blocks` | Yes |
| `parent-child` | Yes |
| `conditional-blocks` | Yes |
| `waits-for` | Yes |
| `related` | No (informational) |
| `discovered-from` | No (provenance) |

**Detection Method:**
- Maintain comma-separated path string during traversal
- Before adding node: check if already in path
- If target found in reachable set: cycle exists

**Use Cases:**
- Prevent creating dependencies that would cause cycles
- Validate dependency graph integrity
- `bd doctor` cycle check

### 8.6 Dependency Tree Building

Builds a hierarchical tree structure from a root issue following its dependencies.

**DependencyNode Structure:**

| Field | Type | Description |
|-------|------|-------------|
| `issue` | Issue | The issue at this node |
| `children` | []DependencyNode | Child nodes (dependencies) |
| `depth` | int | Distance from root (0 = root) |
| `type` | DependencyType | How this node relates to parent |

**Algorithm:**
1. Start at root issue with `depth = 0`
2. Mark root as visited
3. Query dependencies in "down" direction (issues this one depends on)
4. For each dependency not yet visited and within `maxDepth`:
   - Recursively build subtree
   - Attach as child with dependency type
5. Return tree structure

**Constraints:**
- `maxDepth`: Prevents infinite recursion (configurable)
- `visited` set: Prevents revisiting same issue (handles DAGs)

**Output:** JSON-serializable tree for `bd show --tree` and API responses

---

## 9. Configuration System

### 9.1 Configuration Hierarchy

Configuration values are resolved in order (first found wins):

1. **Command-line flags:** `--priority 1`
2. **Environment variables:** `BEADS_PRIORITY=1`
3. **Project config:** `.beads/config.yaml`
4. **Database config:** `config` table
5. **User config:** `~/.config/beads/config.yaml`
6. **Default values:** Hardcoded in code

### 9.2 Config Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `issue_prefix` | string | `"bd"` | Prefix for generated issue IDs |
| `default_priority` | int | `2` | Default priority for new issues |
| `default_type` | string | `"task"` | Default issue type |
| `author_name` | string | git user | Default author/actor name |
| `author_email` | string | git email | Default author email |

**Status/Type Customization:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `status.custom` | string | `""` | Comma-separated custom statuses |
| `types.custom` | string | `""` | Comma-separated custom types |

**Import/Export:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `import.orphan_handling` | string | `"allow"` | How to handle orphan dependencies |
|                         |        |          | - `allow`: Import all |
|                         |        |          | - `skip`: Skip orphan deps |
|                         |        |          | - `strict`: Fail on orphan |
|                         |        |          | - `resurrect`: Create placeholder |
| `export.include_events` | bool | `false` | Include events in JSONL |
| `export.compact_json` | bool | `true` | Minimize JSON whitespace |

**Compaction:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `compaction_enabled` | bool | `false` | Enable AI compaction |
| `compact_tier1_days` | int | `30` | Days before tier-1 compaction |
| `compact_tier2_days` | int | `90` | Days before tier-2 compaction |
| `compact_model` | string | `"claude-3-5-haiku"` | AI model for compaction |
| `compact_max_tokens` | int | `1000` | Max tokens for summary |

**Display:**

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `display.colors` | bool | `true` | Enable colored output |
| `display.unicode` | bool | `true` | Enable Unicode symbols |
| `display.date_format` | string | `"relative"` | Date format: relative, iso, local |

### 9.3 Environment Variables

All config keys can be set via environment variables with `BEADS_` prefix:

```bash
BEADS_ISSUE_PREFIX=proj
BEADS_DEFAULT_PRIORITY=1
BEADS_IMPORT_ORPHAN_HANDLING=strict
```

### 9.4 Config File Format

`.beads/config.yaml`:

```yaml
issue_prefix: proj
default_priority: 1
default_type: feature

status:
  custom: "review,testing"

types:
  custom: "spike,research"

import:
  orphan_handling: allow

display:
  colors: true
  unicode: true
  date_format: relative
```

### 9.5 Metadata Keys

Internal metadata stored in `metadata` table (not user-configurable):

| Key | Description |
|-----|-------------|
| `jsonl_content_hash` | SHA256 of current JSONL file |
| `jsonl_file_hash` | Previous file hash (for change detection) |
| `last_import_time` | RFC3339Nano timestamp of last import |
| `last_export_time` | RFC3339Nano timestamp of last export |
| `schema_version` | Current migration version |
| `workspace_id` | Unique workspace identifier |

---

## 10. Validation Rules

### 10.1 Issue Validation

**Title:**
- Required: Cannot be empty
- Max length: 500 characters
- Trimmed: Leading/trailing whitespace removed

**Description/Design/AcceptanceCriteria/Notes:**
- Optional
- No max length (limited by SQLite TEXT)

**Status:**
- Must be one of: `open`, `in_progress`, `blocked`, `deferred`, `closed`, `tombstone`, `pinned`
- Custom statuses allowed if configured

**Priority:**
- Must be integer 0-4
- Also accepts strings: `P0`, `P1`, `P2`, `P3`, `P4`, `critical`, `high`, `medium`, `low`, `backlog`

**Issue Type:**
- Must be one of: `bug`, `feature`, `task`, `epic`, `chore`, `docs`, `question`
- Custom types allowed if configured

**Timestamps:**
- `created_at`: Set automatically, cannot be changed
- `updated_at`: Set automatically on any change
- `closed_at`: Set automatically when status changes to `closed`
- `deleted_at`: Set automatically when status changes to `tombstone`

**External Ref:**
- Must be unique across all issues (if set)
- Format: Any non-empty string (typically `system-id` like `gh-123`)

### 10.2 Dependency Validation

**Issue IDs:**
- Both `issue_id` and `depends_on_id` must exist (for local deps)
- `depends_on_id` may reference external issue (no FK)

**Type:**
- Must be valid DependencyType
- Blocking types checked for cycles

**Self-Reference:**
- `issue_id` cannot equal `depends_on_id`

**Duplicates:**
- Same (issue_id, depends_on_id) pair cannot exist twice

### 10.3 Label Validation

**Label Name:**
- Cannot be empty
- Max length: 100 characters
- Trimmed: Leading/trailing whitespace removed
- Case-sensitive: `Bug` and `bug` are different labels

### 10.4 Comment Validation

**Author:**
- Required: Cannot be empty

**Text:**
- Required: Cannot be empty
- No max length

### 10.5 ID Validation

**Format:**
- Pattern: `{prefix}-{hash}` (e.g., `bd-abc123`)
- Prefix: Configured via `issue_prefix`
- Hash: 4-8 lowercase hex characters

**Resolution:**
- Exact match first
- Then prefix match (if unique)
- Error if ambiguous (multiple matches)

---

## 11. ID Generation and Content Hashing

### 11.1 Issue ID Generation

IDs are generated using SHA256 hash of:
1. Title
2. Description (first 100 chars)
3. Created timestamp (RFC3339Nano)
4. Workspace ID (from metadata)

```go
func generateIssueID(prefix string, issue *types.Issue, workspaceID string) string {
    h := sha256.New()
    h.Write([]byte(issue.Title))
    h.Write([]byte(issue.Description[:min(100, len(issue.Description))]))
    h.Write([]byte(issue.CreatedAt.Format(time.RFC3339Nano)))
    h.Write([]byte(workspaceID))

    hash := hex.EncodeToString(h.Sum(nil))

    // Progressive length: start at 4 chars, grow as needed
    idLen := 4
    for {
        id := fmt.Sprintf("%s-%s", prefix, hash[:idLen])
        if !idExists(id) {
            return id
        }
        idLen++
        if idLen > 8 {
            // Very rare: fall back to full hash
            return fmt.Sprintf("%s-%s", prefix, hash[:16])
        }
    }
}
```

### 11.2 Hierarchical IDs (Dotted)

Child issues under a parent can use dotted notation:

```
bd-epic1       (parent epic)
bd-epic1.1     (first child)
bd-epic1.2     (second child)
bd-epic1.2.1   (grandchild)
```

Generation uses `child_counters` table:

```go
func generateChildID(parentID string) string {
    // Atomic increment
    result := db.Exec(`
        INSERT INTO child_counters (parent_id, last_child) VALUES (?, 1)
        ON CONFLICT (parent_id) DO UPDATE SET last_child = last_child + 1
        RETURNING last_child
    `, parentID)

    childNum := result.LastChild
    return fmt.Sprintf("%s.%d", parentID, childNum)
}
```

### 11.3 Content Hash Computation

Content hash is SHA256 of normalized issue content, used for:
- Change detection (has issue changed since last export?)
- Deduplication (is this the same issue as another?)
- Collision detection (same ID, different content?)

**Fields included in hash (in order):**

```go
func (i *Issue) ComputeContentHash() string {
    h := sha256.New()

    // 1. Core content
    h.Write([]byte(i.Title))
    h.Write([]byte(i.Description))
    h.Write([]byte(i.Design))
    h.Write([]byte(i.AcceptanceCriteria))
    h.Write([]byte(i.Notes))

    // 2. Status & workflow
    h.Write([]byte(i.Status))
    h.Write([]byte(strconv.Itoa(i.Priority)))
    h.Write([]byte(i.IssueType))

    // 3. Assignment
    h.Write([]byte(i.Assignee))
    h.Write([]byte(i.Owner))
    h.Write([]byte(i.CreatedBy))

    // 4. External
    if i.ExternalRef != nil {
        h.Write([]byte(*i.ExternalRef))
    }
    h.Write([]byte(i.SourceSystem))

    // 5. Close info
    h.Write([]byte(i.CloseReason))
    h.Write([]byte(i.ClosedBySession))

    // 6. Tombstone info
    h.Write([]byte(i.DeletedBy))
    h.Write([]byte(i.DeleteReason))
    h.Write([]byte(i.OriginalType))

    // 7. Flags
    if i.Pinned {
        h.Write([]byte("pinned"))
    }
    if i.IsTemplate {
        h.Write([]byte("template"))
    }
    if i.Ephemeral {
        h.Write([]byte("ephemeral"))
    }

    // 8. Labels (sorted for determinism)
    labels := make([]string, len(i.Labels))
    copy(labels, i.Labels)
    sort.Strings(labels)
    for _, label := range labels {
        h.Write([]byte(label))
    }

    // 9. Dependencies (sorted for determinism)
    deps := make([]string, len(i.Dependencies))
    for j, dep := range i.Dependencies {
        deps[j] = fmt.Sprintf("%s:%s:%s", dep.DependsOnID, dep.Type, dep.Metadata)
    }
    sort.Strings(deps)
    for _, dep := range deps {
        h.Write([]byte(dep))
    }

    return hex.EncodeToString(h.Sum(nil))
}
```

**Fields NOT included (they change without content change):**
- `ID` (identity, not content)
- `ContentHash` (self-referential)
- `CreatedAt`, `UpdatedAt`, `ClosedAt`, `DeletedAt` (metadata)
- `CompactionLevel`, `CompactedAt`, etc. (compaction metadata)
- Internal routing fields (`SourceRepo`, `IDPrefix`, `PrefixOverride`)

---

## 12. Key Architectural Patterns

### 12.1 Non-Invasive Design (br vs bd)

The Rust port (`br`) is designed to be LESS invasive than the Go version (`bd`):

| Feature | `bd` (Go) | `br` (Rust) |
|---------|-----------|-------------|
| Auto git hooks | Yes (installed by default) | No |
| Auto git commit | Yes (after changes) | No |
| Auto git push | Yes (with hooks) | No |
| Background daemon | Yes (default) | No |
| RPC server | Yes | No |
| Auto-import on query | Yes | Yes (simple check) |
| Auto-export after change | Yes (debounced) | Yes (explicit) |

**Explicit Operations Only:**

```bash
# br requires explicit sync
br create "New issue"           # Creates in DB only
br sync --flush-only            # Exports to JSONL
git add .beads/ && git commit   # User's responsibility
git push                        # User's responsibility
```

### 12.2 Last-Touched Issue

Commands that don't specify an issue ID default to the last touched issue:

```bash
br create "Fix bug"     # Creates bd-abc123, sets as last touched
br update --priority 0  # Updates bd-abc123 (implicit)
br show                 # Shows bd-abc123 (implicit)
br close                # Closes bd-abc123 (implicit)
```

**Implementation:**

```go
var lastTouchedID string

func SetLastTouched(id string) {
    lastTouchedID = id
}

func GetLastTouched() string {
    return lastTouchedID
}

func ResolveIssueID(input string) string {
    if input == "" || input == "." {
        return GetLastTouched()
    }
    return input
}
```

### 12.3 Partial ID Resolution

Users can specify partial IDs for convenience:

```bash
br show abc       # Matches bd-abc123 if unique
br show bd-abc    # Also matches bd-abc123
br close abc def  # Closes bd-abc123 and bd-def456
```

**Resolution Algorithm:**

```go
func ResolvePartialID(prefix string) (string, error) {
    // 1. Try exact match
    issue, err := store.GetIssue(ctx, prefix)
    if err == nil {
        return issue.ID, nil
    }

    // 2. Try prefix match
    matches, _ := store.ListIssues(ctx, &IssueFilter{
        IDPrefix: prefix,
        Limit: 2,  // Only need to know if more than 1
    })

    switch len(matches) {
    case 0:
        return "", ErrNotFound
    case 1:
        return matches[0].ID, nil
    default:
        return "", fmt.Errorf("ambiguous ID prefix '%s': matches %d issues", prefix, len(matches))
    }
}
```

### 12.4 Atomic File Operations

All file writes use the atomic write pattern:

1. Write to temporary file in same directory
2. Flush and sync to disk
3. Atomic rename to target path
4. Delete temp file on error

This ensures:
- No partial writes visible
- Power failure safety
- Concurrent read safety

### 12.5 Output Formatting

**Status Icons (Unicode):**

| Status | Icon | Description |
|--------|------|-------------|
| open | ○ | Empty circle |
| in_progress | ◐ | Half-filled circle |
| blocked | ● | Filled circle |
| deferred | ❄ | Snowflake |
| closed | ✓ | Checkmark |
| tombstone | ✗ | X mark |
| pinned | 📌 | Pin |

**Priority Colors:**

| Priority | Color | ANSI Code |
|----------|-------|-----------|
| P0 | Red | `\x1b[31m` |
| P1 | Orange/Yellow | `\x1b[33m` |
| P2 | Blue | `\x1b[34m` |
| P3 | Cyan | `\x1b[36m` |
| P4 | Gray | `\x1b[90m` |

**Tree Rendering:**

```
├── Branch with sibling
│   └── Last child
└── Last branch
    ├── Child 1
    └── Child 2
```

---

## 13. Error Handling

### 13.1 Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum BeadsError {
    #[error("issue not found: {0}")]
    NotFound(String),

    #[error("invalid issue ID: {0}")]
    InvalidID(String),

    #[error("ambiguous ID prefix '{0}': matches {1} issues")]
    AmbiguousID(String, usize),

    #[error("dependency cycle detected")]
    CycleDetected,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("locked: database is locked")]
    Locked,
}
```

### 13.2 Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Issue not found |
| 4 | Validation error |
| 5 | Database error |
| 6 | Cycle detected |
| 7 | Conflict |

### 13.3 Error Messages

Follow this format for user-facing errors:

```
Error: <brief description>

<detailed explanation if needed>

Hint: <suggestion for resolution>
```

Example:

```
Error: Issue not found: bd-xyz

No issue matches the ID 'bd-xyz'.

Hint: Use 'br list' to see available issues, or check the ID spelling.
```

---

## 14. Porting Considerations

### 14.1 Rust Type Mapping

| Go Type | Rust Type | Notes |
|---------|-----------|-------|
| `string` | `String` | |
| `*string` | `Option<String>` | |
| `int` | `i32` | SQLite INTEGER is i64, but priority is 0-4 |
| `int64` | `i64` | For IDs, counts |
| `*int` | `Option<i32>` | |
| `float32` | `f32` | |
| `*float32` | `Option<f32>` | |
| `float64` | `f64` | |
| `bool` | `bool` | |
| `time.Time` | `DateTime<Utc>` | chrono crate |
| `*time.Time` | `Option<DateTime<Utc>>` | |
| `time.Duration` | `std::time::Duration` | |
| `[]string` | `Vec<String>` | |
| `[]T` | `Vec<T>` | |
| `map[string]T` | `HashMap<String, T>` | |
| `map[string]interface{}` | `HashMap<String, serde_json::Value>` | |
| `error` | `Result<T, BeadsError>` | |
| `context.Context` | Implicit or `&self` | |

### 14.2 Serde Configuration

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub content_hash: String,

    pub title: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    pub priority: i32,  // Never skip, 0 is valid

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,

    #[serde(default, skip_serializing_if = "Option::is_none", with = "chrono::serde::ts_seconds_option")]
    pub closed_at: Option<DateTime<Utc>>,
}
```

### 14.3 Key Crates

| Purpose | Crate | Version |
|---------|-------|---------|
| CLI parsing | `clap` | 4.x with derive |
| SQLite | `rusqlite` | Latest, bundled feature |
| JSON | `serde` + `serde_json` | Latest |
| Time | `chrono` | Latest, serde feature |
| Hashing | `sha2` | Latest |
| Parallel | `rayon` | Latest |
| Logging | `tracing` | Latest |
| Errors | `anyhow` + `thiserror` | Latest |
| Colors | `colored` or `termcolor` | Latest |
| Tables | `comfy-table` or `tabled` | Latest |

### 14.4 Schema Compatibility

**Critical:** The Rust implementation MUST use the same SQLite schema as Go beads. This allows:
- Cross-tool usage (run `bd` and `br` on same `.beads/`)
- Migration from Go to Rust without data conversion
- Shared JSONL format for git sync

**Schema Verification:**

```rust
fn verify_schema_compatibility(conn: &Connection) -> Result<()> {
    // Check all required tables exist
    let required_tables = [
        "issues", "dependencies", "labels", "comments", "events",
        "config", "metadata", "dirty_issues", "export_hashes",
        "blocked_issues_cache", "child_counters", "schema_migrations"
    ];

    for table in required_tables {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
            [table],
            |row| row.get(0)
        )?;

        if !exists {
            return Err(anyhow!("Missing required table: {}", table));
        }
    }

    Ok(())
}
```

### 14.5 Output Compatibility

JSON output must be character-for-character identical to Go beads for machine consumers:

```rust
// Use serde_json with these settings
let json = serde_json::to_string(&issue)?;  // Compact, no pretty-print

// For JSONL, one object per line, no trailing newline on last line
for issue in issues {
    writeln!(file, "{}", serde_json::to_string(&issue)?)?;
}
```

### 14.6 Priority Order for Implementation

1. **Phase 1: Core Data Types**
   - Issue, Dependency, Comment, Event structs
   - Status, IssueType, DependencyType enums
   - Validation functions
   - Content hash computation

2. **Phase 2: SQLite Storage**
   - Connection management with pragmas
   - Schema creation and migration
   - Basic CRUD operations
   - Dirty tracking
   - Blocked cache

3. **Phase 3: Basic CLI**
   - `init`, `create`, `update`, `close`
   - `list`, `show`, `ready`, `blocked`
   - `stats`, `config`

4. **Phase 4: Dependencies**
   - `dep add`, `dep remove`, `dep list`
   - `dep tree`, `dep cycles`
   - Cycle detection
   - Blocked cache refresh

5. **Phase 5: JSONL System**
   - Export flow
   - Import flow
   - Dirty tracking
   - Collision detection

6. **Phase 6: Sync Command**
   - `sync --flush-only`
   - `sync --import-only`
   - Status reporting

7. **Phase 7: Polish**
   - `search` command
   - `label` command
   - Output formatting
   - Error messages
   - Documentation

---

## Appendix A: Important Invariants

### Closed-At Invariant

```
IF status == "closed" THEN closed_at MUST be set (non-NULL)
IF status NOT IN ("closed", "tombstone") THEN closed_at MUST be NULL
```

Enforced by SQLite CHECK constraint.

### Tombstone Invariant

```
IF status == "tombstone" THEN deleted_at MUST be set
IF status != "tombstone" THEN deleted_at SHOULD be NULL
```

### Priority Range

```
0 <= priority <= 4
```

Enforced by SQLite CHECK constraint.

### Title Length

```
1 <= len(title) <= 500
```

Enforced by SQLite CHECK constraint and application validation.

### Cycle Prevention

Blocking dependencies (`blocks`, `parent-child`, `conditional-blocks`, `waits-for`) cannot form cycles. Enforced by application logic before insert.

### ID Uniqueness

All issue IDs must be unique. Enforced by PRIMARY KEY constraint.

### External Ref Uniqueness

All non-NULL external_ref values must be unique. Enforced by UNIQUE index.

---

## 15. Additional Legacy Findings (2026-01-16)

This section captures additional behaviors discovered in the legacy codebase, expressed as implementation guidance for the Rust port. These are intentionally **synthesized** (not verbatim code) and focused on the non-Gastown feature set.

### 15.1 Transaction Semantics (Storage Interface)

The storage layer exposes both a full `Storage` interface and a narrower `Transaction` interface. A transaction is expected to:

- Use **BEGIN IMMEDIATE** for SQLite (acquire write lock early, avoid deadlocks).
- Provide **read-your-writes** within the transaction (e.g., `GetIssue`/`SearchIssues` reflect prior calls in the same txn).
- Roll back on **any error** or **panic**, commit only on clean return.
- Be used for multi-step operations (create issue + dependencies + labels + events) to ensure atomicity.

The interface also allows **UnderlyingDB/UnderlyingConn** access for extensions. For `br`, keep these as *optional escape hatches* but avoid using them directly in core commands.

### 15.2 ID Generation: Base36 + Adaptive Length

The effective ID scheme in current legacy beads is **base36**, not hex:

- Hash material: `title | description | creator | created_at (ns) | nonce`.
- Encoding: SHA256 → base36 string of **length 3–8**.
- Collision handling: for each length, try **up to 10 nonces**, then increase length.
- **Adaptive length** is computed from DB size using a birthday-paradox threshold (default 25% collision probability). Config keys:
  - `max_collision_prob`
  - `min_hash_length`
  - `max_hash_length`
- Hierarchical children use **parent.N** with a `child_counters` table; maximum depth defaults to 3.

**Porting note:** This supersedes earlier “hex short ID” assumptions. `br` should implement base36 adaptive length for true parity.

### 15.3 Partial ID Resolution (CLI Behavior)

Short ID resolution is more permissive than strict prefix matching:

- Accepts full IDs, bare hashes, and prefixes without hyphen.
- Attempts exact match first; then exact match on normalized prefix; then **substring match** across all issue IDs.
- If ambiguous, returns an explicit “ambiguous ID” error listing candidates.

This behavior matters for UX parity and test harness compatibility.

### 15.4 Ready/Blocked Semantics and Cache

Ready work uses a **materialized cache** (`blocked_issues_cache`) instead of a recursive CTE:

Blocking types handled:
- `blocks`
- `conditional-blocks` (blocked unless dependency closes with a **failure** reason)
- `waits-for` (blocked until children of a “spawner” close; gate types: `all-children` or `any-children`)
- `parent-child` (transitive propagation down the hierarchy)

Key behaviors:
- **Pinned** issues are excluded from ready work.
- **Ephemeral** issues (wisps) are excluded; `*-wisp-*` IDs are a defense-in-depth filter.
- Default ready filter includes `open` and `in_progress`.
- `hooked` status counts as blocking in the cache (even if Gastown is excluded).

Cache invalidation:
- Rebuild is triggered on dependency changes (blocking types only) and on status changes.
- Rebuild is **full** (DELETE + INSERT) within the same transaction for consistency.

### 15.5 Conditional-Blocks Failure Heuristic

For `conditional-blocks`, “failure” is detected by **close_reason keywords**:

`failed`, `rejected`, `wontfix`, `won't fix`, `cancelled`, `canceled`, `abandoned`, `blocked`, `error`, `timeout`, `aborted`

If the blocker closes **without** failure, the dependent remains blocked.

### 15.6 External Dependency Resolution

External dependencies are encoded as:

```
external:<project>:<capability>
```

They are satisfied when the referenced project has a **closed** issue labeled:

```
provides:<capability>
```

Resolution details:
- Projects are configured via `external_projects` in config.
- The check opens the external project’s `.beads` DB **once per project** (batched by capability).
- External deps are **not** included in blocked cache rebuilds; they are evaluated at query time.

### 15.7 JSONL File Selection (Safety)

When locating a JSONL file in `.beads/`, legacy code:

1. **Prefers** `issues.jsonl` (canonical).
2. Falls back to `beads.jsonl` (legacy).
3. Otherwise uses any `.jsonl` **except**:
   - `deletions.jsonl`
   - `interactions.jsonl`
   - merge artifacts: `beads.base.jsonl`, `beads.left.jsonl`, `beads.right.jsonl`

This avoids accidental reads of conflict files or deletions logs.

### 15.8 Import Pipeline Details (High-Fidelity)

Import flow (non-daemon and daemon paths both use this core logic):

- **Normalize**:
  - Recompute `content_hash` for all incoming issues (trust DB, not JSONL).
  - If ID contains `-wisp-`, set `ephemeral = true` (prevent ready pollution).
  - Canonicalize **Linear** external refs to avoid duplicates.
- **Prefix handling**:
  - If multi-repo config is active, prefix validation is **skipped**.
  - Otherwise, prefixes are validated against `issue_prefix` and `allowed_prefixes`.
  - Tombstoned issues with mismatched prefixes are tolerated (do not block import).
- **Collision handling**:
  - Uses the 4-phase matching algorithm already summarized in §7 (external_ref → content_hash → ID → new).
  - Tombstones never resurrect.
  - Timestamp protection (`protect_local_export_ids`) can skip older updates.
- **Write path**:
  - Clear `export_hashes` before import (prevents stale dedup).
  - Import via transaction; dependencies + labels + comments follow.
  - WAL checkpoint is attempted post-import (non-fatal if it fails).

### 15.9 Foreign Keys and Orphan Dependencies

To allow `external:*` dependencies, legacy migrations **remove the FK** on `depends_on_id`. As a result:

- Imports temporarily disable foreign keys for speed and order-independence.
- A **manual orphan check** is run post-import:
  - Any dependency whose `depends_on_id` doesn’t exist **and** does not start with `external:` is treated as an error.

**Porting note:** Keep this behavior; do not reintroduce strict FK on `depends_on_id`.

### 15.10 Multi-Repo Hydration (Read-Only Aggregation)

When `.beads/config.yaml` includes `repos.primary`/`repos.additional`, the SQLite storage hydrates issues from multiple repos:

- Reads each repo’s `.beads/issues.jsonl` and merges into the primary DB.
- Tracks JSONL **mtime** in `repo_mtimes` to skip unchanged repos.
- Uses **Lstat** (not Stat) to respect symlinks (NixOS fix).
- Sets `source_repo` on imported issues to the repo-relative path.

### 15.11 Validation Guards in Mutating Commands

Mutation operations are guarded by a composable validator chain:

- **Exists** check (error: issue not found).
- **NotTemplate** (templates are read-only).
- **NotPinned** unless `--force`.
- **NotHooked** unless `--force`.
- **NotClosed** (for close / state transitions).

**Porting note:** These checks should remain even if Gastown features are excluded; “pinned” and “template” are still part of the classic model.

### 15.12 Issue Listing and Search Semantics (SQLite)

Legacy "list" behavior is effectively `SearchIssues` with filters; there is no separate list-specific query. Important characteristics:

- **Text search** uses simple `LIKE` (no FTS):
  - Query string matches `title`, `description`, or `id`.
  - `TitleSearch` is a **title-only** `LIKE`.
  - `TitleContains`, `DescriptionContains`, `NotesContains` each add additional `LIKE` clauses.
- **Default tombstone behavior**: if `Status` is not explicitly set and `IncludeTombstones` is false, tombstones are excluded (`status != tombstone`).
- **No implicit filtering** of `ephemeral`, `pinned`, or `template` issues unless the filter explicitly requests it.
- **Labels**:
  - `Labels` is AND semantics (issue must have **all** labels).
  - `LabelsAny` is OR semantics (issue must have **at least one** label).
- **ID matching**:
  - `IDs` is a strict `IN` list.
  - `IDPrefix` is `LIKE 'prefix%'`.
- **Scheduling filters**: `defer_until` and `due_at` are compared using RFC3339 strings.
  - `Overdue` means `due_at < now AND status != closed`.
- **Ordering**: results are ordered by **priority ASC**, then **created_at DESC** (newer first within priority).
- **Pagination**: `Limit` is supported; no offset in this path.
- **Relations**: `SearchIssues` returns issue rows only; it does **not** populate labels/dependencies/comments.
- **Daemon safety**: a read lock is held during queries to avoid reconnect races (daemon mode).

**Porting note:** This ordering and the lack of implicit `ephemeral`/`pinned` filtering can diverge from UX expectations; preserve for parity, then consider optional improvements.

### 15.13 Statistics Semantics (SQLite)

`GetStatistics` is computed via aggregate SQL with some important quirks:

- `TotalIssues` excludes tombstones; tombstones are counted separately.
- `PinnedIssues` are counted explicitly.
- `DeferredIssues` are counted explicitly.
- `BlockedIssues` count is based **only** on `blocks` dependencies (not conditional-blocks, waits-for, or parent-child).
- `ReadyIssues` count uses a simple rule:
  - status = `open`
  - no `blocks` dependency whose blocker is in `open|in_progress|blocked|deferred|hooked`
  - (Note: this does **not** use the blocked cache, and does not consider conditional-blocks or waits-for.)
- `AverageLeadTime` is computed in hours via `julianday(closed_at) - julianday(created_at)`.
- `EpicsEligibleForClosure` counts epics where **all** children are closed.

**Porting note:** These stats intentionally do **not** share the full ready/blocked logic. Maintain this behavior for parity; if we want to reconcile in `br`, it should be a deliberate change.

### 15.14 Import Collision Resolution (Detailed)

The importer uses a **content-first** merge strategy with several concrete rules:

- **Batch de-dup**:
  - Skip duplicate incoming issues by **content hash** (first wins).
  - Skip duplicate incoming issues by **ID** (first wins).
- **Tombstone protection**:
  - If DB has a tombstone for the incoming ID, skip import **before** any other match path.
- **External ref priority**:
  - `external_ref` match is checked **before** ID or content hash.
  - Updates occur only if `incoming.updated_at > existing.updated_at`.
  - `assignee` and `external_ref` are cleared if incoming is empty.
  - `pinned` is only updated if incoming explicitly has `pinned=true` (false is treated as "no change").
- **Content hash match**:
  - Same content + same ID => idempotent (unchanged).
  - Same content + different ID:
    - If **different prefixes**, treat as cross-project duplicate and **skip**.
    - If **same prefix**, treat as a rename (handled by `handleRename`).
- **ID collision (new content)**:
  - Treated as an **update** if incoming is newer (timestamp check).
  - If older or equal, skip or mark unchanged.
- **Timestamp-aware protection (GH#865)**:
  - If a local export timestamp for the ID is newer, the update is blocked.
- **Creation ordering**:
  - New issues are sorted by hierarchy depth, then created depth-by-depth (0..3).
  - This guarantees parents exist before children.
- **Orphan handling**:
  - In `orphan_skip` mode, hierarchical children with missing parents are filtered out **before** creation.

**Porting note:** These rules are subtle but critical for sync correctness, especially in multi-clone workflows.

### 15.15 CLI Flag Semantics (Classic Commands)

This subsection captures flag behavior for the **core** non-Gastown CLI commands. We only include flags relevant to classic beads; Gastown-specific flags are intentionally omitted from the Rust port.

#### create

Accepted inputs:
- Title is positional OR `--title`; if both are set they must match.
- `--file` parses a markdown file to create multiple issues; `--dry-run` is **not** supported with `--file`.

Core flags:
- `--type` (default `task`), `--priority` (default `2`, supports `P0..P4`).
- Common content flags: `--description`, `--body` (alias), `--body-file`/`--description-file` (file input), `--design`, `--acceptance`, `--notes`.
- `--labels` / `--label` (alias); labels are normalized (trim + dedupe).
- `--id` explicit ID, `--parent` hierarchical child, `--deps` dependency list, `--external-ref`.
- `--due`, `--defer` support relative/natural time; `--defer` warns if in the past.
- `--silent` prints only ID; otherwise prints a summary and sets “last touched”.
- `--validate` optionally enforces required description sections (config driven).

Behavioral notes:
- Creating title starting with “test” triggers a warning unless `--silent`.
- If `create.require-description` is enabled, `--description` is mandatory (unless “test” issue).
- After success: marks dirty, schedules auto-flush, fires hooks, sets last touched ID.

#### update

Accepted inputs:
- Accepts multiple IDs; if none, uses “last touched”.
- Uses partial ID resolution (same rules as show/close).

Core flags:
- `--status`, `--priority`, `--title`, `--assignee`, `--type`, `--estimate`.
- Common content flags: `--description/--body/--body-file`, `--design`, `--acceptance`, `--notes`.
- `--add-label`, `--remove-label`, `--set-labels`.
- `--parent` reparent (empty string removes parent).
- `--due`, `--defer` (empty string clears).
- `--claim` sets assignee to actor and status to `in_progress` **atomically** (fails if already claimed).
- `--session` supports closed status attribution.

Behavioral notes:
- Only changed flags are applied (e.g., P0 must be detected via `Flags().Changed`).
- After updates: marks dirty + schedules flush, sets last touched to first updated ID, fires hooks.

#### close

Accepted inputs:
- Accepts multiple IDs; if none, closes “last touched”.
- Partial ID resolution; cross-repo routing is honored in direct mode.

Core flags:
- `--reason` (default “Closed”), `--resolution` alias.
- `--force` bypasses pinned/hooked checks.
- `--suggest-next` outputs newly-unblocked issues.
- `--continue` + `--no-auto` advances to next step (molecule flow).
- `--session` sets `closed_by_session`.

Behavioral notes:
- If not forced, **close is blocked** if issue has open blockers.
- Closing triggers hook, marks dirty, schedules flush.
- `--continue` requires direct DB access; daemon mode prints a hint.

#### list

Core flags (subset for classic port):
- Filters: `--status`, `--assignee`, `--type`, `--label`, `--label-any`, `--id`, `--priority`, `--priority-min`, `--priority-max`.
- Text/pattern: `--title`, `--title-contains`, `--desc-contains`, `--notes-contains`.
- Date ranges: `--created-after|before`, `--updated-after|before`, `--closed-after|before`.
- Empty checks: `--empty-description`, `--no-assignee`, `--no-labels`.
- Scheduling: `--deferred`, `--defer-after|before`, `--due-after|before`, `--overdue`.
- Formatting: `--long`, `--pretty|--tree`, `--format` (template/dot/digraph), `--no-pager`.
- Behavior: `--all` (include closed), `--ready` (open-only shortcut), `--limit` (0=unlimited, agent-mode default=20).

Behavioral notes:
- Default filter excludes **closed** unless `--all` or explicit `--status`.
- Directory-aware label scoping applies only if **no** labels were specified and config has `directory.labels`.
- Pretty/tree uses dependency graph if available; falls back to dotted ID hierarchy.
- In JSON mode, `IssueWithCounts` is returned (dep counts embedded).

#### search

- Requires a query (positional or `--query`).
- Filter flags largely mirror list (status, assignee, type, labels, priority ranges, date ranges, limit, sort).
- Uses the **same SearchIssues** backend as list (LIKE-based search). Results are sorted after retrieval.

#### show

- Accepts multiple IDs; supports `--short`, `--thread`, `--refs`, `--children`.
- JSON output returns **IssueDetails** (issue + labels + deps + dependents + comments + parent).
- Text output includes header + metadata + markdown-rendered description.

#### ready / blocked

- `ready` filters: `--assignee`, `--unassigned`, `--priority`, `--label`, `--label-any`, `--type`, `--parent`, `--mol-type`, `--limit`, `--sort` (hybrid|priority|oldest), `--include-deferred`.
- `ready --gated` and molecule-specific flows are out of scope for `br` classic port.
- `blocked` is a direct read of blocked cache; supports `--parent`.

### 15.16 Config and Environment Precedence (Classic)

Configuration sources are merged in this order (highest wins):

1) **Flags**
2) **Environment variables** (`BD_*`, and select `BEADS_*`)
3) **Project `.beads/config.yaml`**
4) **User config** (`~/.config/bd/config.yaml`)
5) **Legacy user config** (`~/.beads/config.yaml`)
6) **Defaults**

Key behaviors:
- “Startup” keys (e.g., `no-db`, `no-daemon`, `json`, `lock-timeout`, `sync.branch`, `routing.*`, `directory.labels`, `external_projects`, `validation.*`) are **YAML-only** and cannot be stored in SQLite.
- `bd config set/get` will write to YAML for yaml-only keys and to the DB for others.
- `config list` warns when YAML/env overrides DB values (notably `sync.branch`).

### 15.17 Routing and Multi-Repo Resolution (Classic)

Routing is split into **creation routing** and **ID routing**:

**Creation routing** (where new issues go):
- `routing.mode` (`auto` or `explicit`) and `routing.default/maintainer/contributor` are read from config.yaml.
- In `auto` mode, role detection uses git config and remote write URLs to infer maintainer vs contributor.
- `--repo` (explicit) or `--rig`/`--prefix` overrides routing decisions.

**ID routing** (where to read/update existing issues):
- `routes.jsonl` defines `prefix -> path` mappings.
- Resolution searches local `.beads/routes.jsonl` first, then walks up to a **town root** (identified by `mayor/town.json`) and uses `<townRoot>/.beads/routes.jsonl`.
- A per-rig `redirect` file inside `.beads/` can override the target path.
- Routing is used by `show`, `close`, `update`, and other commands that accept IDs.

**External ref derivation:** if an ID prefix matches a route, it can be converted to `external:<project>:<id>` for cross-project dependency resolution.

### 15.18 Sync, Auto-Import, and Auto-Flush (Classic Behavior)

Legacy `bd` sync + background behaviors combine three layers: **explicit sync**, **auto-import on reads**, and **auto-flush on writes**.

**Explicit sync modes (`bd sync`):**
- `--flush-only`: export pending changes to JSONL and exit (no git ops in this mode).
- `--import-only`: import JSONL into DB and exit (no git ops in this mode).
- `--dry-run`: print actions without touching the DB or JSONL.
- Sync forces **direct mode** (daemon bypass) to avoid stale daemon connections.

**Auto-flush (writes):**
- Mutating commands call `markDirtyAndScheduleFlush()` which **debounces** JSONL export.
- A final flush is attempted in `PersistentPostRun` unless explicitly skipped by sync.
- Flush uses atomic temp-file rename and updates metadata (`jsonl_content_hash`, `last_export_time`).

**Auto-import (reads):**
- Read-only commands call `ensureDatabaseFresh()` unless daemon mode.
- If JSONL is newer and `--no-auto-import` is **false**, it runs auto-import and continues.
- If JSONL is newer and `--no-auto-import` is **true**, the command aborts with guidance.
- `--allow-stale` skips the freshness check entirely (warns to stderr).

**Auto-import details:**
- JSONL staleness uses **hash comparison** (metadata key `jsonl_content_hash`, fallback `last_import_hash`).
- Detects Git conflict markers and aborts with a conflict-resolution hint.
- Normalizes issues (default values; sets `closed_at` if missing for closed issues).
- Uses import options: `SkipPrefixValidation=true`, `Strict=false`.
- Clears export hashes before import.
- After import, schedules a **flush** (full export if ID remaps happened).

**Porting note:** `br` should preserve **flush-only** and **import-only** semantics, but **omit git operations**. Auto-flush/import behaviors are part of classic UX and should remain, gated by explicit flags.

### 15.19 Classic-Only Command Subset (Scope Table)

The Rust port targets the **classic** issue tracker (SQLite + JSONL) and intentionally omits Gastown/daemon/hook automation. This table defines the initial scope.

**Included in `br` v1 (classic parity):**
- Init & core CRUD: `init`, `create`, `update`, `close`, `reopen`, `delete` (tombstone)
- Views & queries: `list`, `show`, `ready`, `blocked`, `search`, `stats`, `stale`
- Structure: `dep`, `label`, `comments`
- Sync: `sync --flush-only`, `sync --import-only` (no git ops)
- Config: `config get/set/list/unset` (yaml-only vs DB-backed)

**Explicitly excluded in `br` v1:**
- Gastown features: `gate`, `agent`, `molecule`, `rig`, `convoy`, `hop`, `session`
- Daemon / RPC / auto-git hooks / auto-commit / auto-push
- Linear/Jira integrations
- TUI or visualization features (delegated to `bv`)

### 15.20 Core Flag Matrix (Classic Commands)

This matrix is a compact reference for core flags. It is not exhaustive, but it captures the flags that most directly affect semantics or output in classic mode.

| Command | Core Flags | Notes |
|---|---|---|
| `create` | `--title`, `--type`, `--priority`, `--assignee`, `--description/--body/--body-file`, `--design`, `--acceptance`, `--notes`, `--labels/--label`, `--parent`, `--deps`, `--external-ref`, `--id`, `--due`, `--defer`, `--silent`, `--dry-run` | `--file` creates multiple issues (no `--dry-run`); “test” titles warn; description can be required by config |
| `update` | `--status`, `--priority`, `--title`, `--assignee`, `--type`, `--estimate`, `--description/--body/--body-file`, `--design`, `--acceptance`, `--notes`, `--add-label`, `--remove-label`, `--set-labels`, `--parent`, `--due`, `--defer`, `--claim`, `--session` | Only changed flags apply; `--claim` is atomic |
| `close` | `--reason`, `--force`, `--continue`, `--no-auto`, `--suggest-next`, `--session` | Blocks if open blockers unless `--force` |
| `list` | `--status`, `--assignee`, `--type`, `--label`, `--label-any`, `--id`, `--priority`, `--priority-min/max`, `--title`, `--title-contains`, `--desc-contains`, `--notes-contains`, `--created/updated/closed-*`, `--defer/due-*`, `--overdue`, `--empty-description`, `--no-assignee`, `--no-labels`, `--all`, `--ready`, `--limit`, `--pretty/--tree`, `--long`, `--format`, `--no-pager` | Default excludes closed; `--limit 0` = unlimited; JSON returns IssueWithCounts |
| `search` | `--query`, `--status`, `--assignee`, `--type`, `--label`, `--label-any`, `--priority-min/max`, `--created/updated/closed-*`, `--limit`, `--sort`, `--reverse`, `--long` | Uses same backend as list; query is required |
| `show` | `--short`, `--thread`, `--refs`, `--children` | JSON returns IssueDetails |
| `ready` | `--assignee`, `--unassigned`, `--priority`, `--label`, `--label-any`, `--type`, `--parent`, `--mol-type`, `--limit`, `--sort`, `--include-deferred` | `--gated` and molecule routing are Gastown-only (exclude) |
| `blocked` | `--parent` | Reads blocked cache |

### 15.21 `br` Compatibility Checklist (Classic)

Use these assertions to validate parity with `bd` (classic, non-Gastown). Each item should map to a conformance test.

- **ID generation**: base36 adaptive length + nonce collision retries; hierarchical child IDs via `child_counters`.
- **Partial ID resolution**: exact → normalized → substring; ambiguous errors list candidates.
- **Search/List semantics**: tombstone exclusion default; label AND/OR; ordering by priority ASC then created_at DESC.
- **Ready semantics**: blocked cache usage, pinned/ephemeral excluded, `defer_until` respected.
- **Close semantics**: cannot close with open blockers unless forced; sets `closed_at`, `close_reason`, `closed_by_session`.
- **Import collision rules**: external_ref priority, tombstone protection, timestamp-aware protection.
- **JSONL format**: one issue per line (json.Encoder adds a newline per record); skip ephemeral; include tombstones.
- **Auto-import**: hash-based staleness check; conflict marker abort; skip when `--no-auto-import` set.
- **Auto-flush**: debounced, atomic write, final flush on exit unless sync says otherwise.
- **Config precedence**: flags > env > project config > user config > defaults; yaml-only keys honored.
- **Routing**: routes.jsonl + redirect behavior for cross-repo IDs; external refs derived from routes.

### 15.22 Deletion, Tombstones, and Reference Hygiene

Legacy deletion is **tombstone-first** with optional hard pruning:

- Default `delete` creates **tombstones** (status = `tombstone`, sets `deleted_at`, `deleted_by`, `delete_reason`).
- Tombstones are exported to JSONL to prevent resurrection after sync.
- `--hard` **only prunes tombstones from JSONL**; the DB tombstone remains until a separate cleanup step.

Deletion safety model:
- Without `--force`, `bd delete` shows a **preview** and exits.
- Preview lists dependency links that would be removed and connected issues whose text would be rewritten.
- `--cascade` recursively deletes dependents; `--force` can orphan dependents if not cascading.
- `--from-file` accepts one ID per line (skips empty/commented lines).

Reference hygiene:
- After deletion, connected issues are updated to replace plain-text references to deleted IDs with `[deleted:<id>]`.
- Replacement is applied to description/notes/design/acceptance_criteria using a boundary-aware regex that treats hyphenated IDs as whole tokens.

Porting note:
- For `br`, keep tombstone behavior and text reference rewriting; hard delete should be explicitly scoped and **not** resurrectable via sync.

### 15.23 Reopen and Comments (Classic UX)

**Reopen**:
- `reopen` explicitly sets status to `open` and clears `closed_at` (distinct from `update --status open`).
- If a `--reason` is provided, it is added as a **comment**.
- Supports multiple IDs, partial ID resolution, and JSON output (array of reopened issues).

**Comments**:
- `comments <issue-id>` lists comments; JSON output is an array.
- `comments add <issue-id> <text>` or `-f <file>` adds a comment.
- Author is from `--author` or falls back to actor (git-aware identity).
- There is a hidden alias `comment` for backward compatibility.

Porting note:
- Comments are part of the core sync surface (export/import) and should be preserved.

### 15.24 Stale and Count Views

**Stale**:
- Shows issues not updated in `N` days (default 30), optionally filtered by status.
- In JSON mode, returns a list of issues; in text mode, prints a ranked list with “days stale”.

**Count**:
- Counts issues matching list-like filters (status/type/labels/date ranges/priority ranges, etc.).
- Optional group-by: `status`, `priority`, `type`, `assignee`, or `label`.
- When grouped by label, each issue contributes to **each** label it has; unlabeled issues are grouped under `(no labels)`.

Porting note:
- These are read-only views but rely on the same `IssueFilter` semantics as list/search.

### 15.25 Dependency Tree and Cycle Detection Details

**Dependency tree output shape**:
- `GetDependencyTree` returns a **flat list** of `TreeNode` records, not a nested tree.
- Each node includes `Depth` (root = 0) and `ParentID` to allow UI tree reconstruction.
- Rows are ordered by `depth`, then `priority`, then `id` for stable rendering.
- `maxDepth <= 0` defaults to **50**; nodes at `depth == maxDepth` are flagged `Truncated = true`.

**Traversal direction**:
- Normal mode (`reverse=false`) walks **dependencies** (what blocks this issue).
- Reverse mode (`reverse=true`) walks **dependents** (what depends on this issue).
- External refs are appended **only** in normal mode (dependencies); reverse mode shows local dependents only.

**Cycle safety and substring bug fix**:
- Recursive CTE tracks a `path` string with a `→` delimiter to avoid cycles.
- Path exclusion checks are **boundary-aware** (avoids false positives like `bd-10` containing `bd-1`).
- Result: the classic substring bug is explicitly covered by tests and blocked by path matching.

**Dedup vs full paths**:
- `showAllPaths=false` (default) deduplicates nodes by ID; the **shallowest occurrence wins**.
- `showAllPaths=true` keeps duplicates so diamond-shaped graphs show **all paths**.

**External dependencies in the tree**:
- External refs (`external:project:capability`) are **synthetic leaf nodes**.
- Each external node uses status from `CheckExternalDep`:
  - satisfied → `closed`, title prefixed with `✓`
  - unsatisfied → `blocked`, title prefixed with `⏳`
- External nodes are added at `parentDepth + 1` and skipped if parent is already at `maxDepth`.

**Cycle detection policy**:
- `AddDependency` prevents cycles via a recursive CTE with `maxDependencyDepth = 100`.
- All dependency types participate **except** `relates-to` (intentionally bidirectional).
- Cross-type cycles are blocked (e.g., `blocks` + `parent-child` + `discovered-from`).
- Self-dependencies are rejected; parent-child direction is validated (parent cannot depend on child).
- External refs do not participate in cycles (they cannot be traversed).
- `DetectCycles` exists for reporting: DFS over non-`relates-to` edges, with cycle normalization.

Porting note:
- Reproduce **flat list semantics**, depth/defaults, dedup behavior, and external node synthesis.
- Preserve cycle-prevention semantics and the substring-safe path filtering.

### 15.26 JSONL Export Edge Cases and Integrity

**Safety guard (empty DB)**:
- Full export refuses to overwrite a **non-empty JSONL** if the database has **zero issues**.
- This prevents catastrophic overwrites from empty/corrupt DB states.

**Canonical export content**:
- Always includes tombstones; always excludes ephemerals/wisps.
- Always sorts by ID for deterministic output.
- Uses `json.Encoder` (one JSON object per line, newline after each record).

**Atomic write + permissions**:
- Export writes to a temp file in the same directory, then renames atomically.
- Single-repo export sets permissions to **0600** (rw-------).
- Auto-flush and multi-repo exports set permissions to **0644** (rw-r--r--), and skip chmod for symlinks.
- Multi-repo export writes **empty JSONL** for repos with zero issues to keep clones in sync.
- Relative repo paths resolve **from repo root**, not CWD (prevents `.beads/oss/.beads` mistakes).

**Deferred finalize (sync atomicity)**:
- `exportToJSONLDeferred` writes JSONL but **does not** update metadata.
- `finalizeExport` runs only after git commit, then:
  - clears `dirty_issues`,
  - updates `jsonl_content_hash` and `last_import_time`,
  - touches DB mtime to be ≥ JSONL mtime.
- Metadata failures are warnings only (export still succeeds).

**Auto-flush integrity check**:
- Stored `jsonl_file_hash` is compared to current JSONL content hash.
- If JSONL is missing or hash mismatches, **export_hashes are cleared** and `jsonl_file_hash` is reset.
- This forces a **full export** and prevents perpetual mismatch warnings.

**Export policies and manifests**:
- Error policy: `strict`, `best-effort`, `partial`, `required-core` (auto-export defaults to best-effort).
- Labels/comments are treated as **enrichments**; policy may allow exporting core data without them.
- `export.skip_encoding_errors=true` skips bad issues and records warnings.
- `export.write_manifest=true` writes `.manifest.json` next to the JSONL with warnings/partials/failed IDs.
- Manifest writes are atomic and set permissions to **0600**.

Porting note:
- Preserve the empty-DB safety check, atomic writes, permission modes, hash tracking, and
  the **integrity-driven full export** fallback when JSONL content diverges.

---

### 15.27 Adaptive Hash IDs, Prefix Heuristics, and Partial Resolution

**Adaptive base36 hash IDs (top-level only)**:
- Hash IDs use **base36** (0-9, a-z) with adaptive length **3-8** characters.
- Adaptive length is computed from **top-level issue count only** (children are excluded by checking for no dot suffix after the prefix).
- Collision probability is estimated with a birthday-paradox approximation; the first length whose probability is <= threshold is chosen.
- Config keys (DB-backed):  
  - `max_collision_prob` (default 0.25)  
  - `min_hash_length` (default 3)  
  - `max_hash_length` (default 8)
- If adaptive length lookup fails, fallback length is **6**.

**Hash ID generation**:
- Candidate ID = `prefix + "-" + base36(hash(title|description|creator|created_at_unix_nano|nonce))`.
- For each length from `baseLength..8`, try **nonce 0..9** and check for collisions in DB.
- Batch ID generation tracks **intra-batch collisions** using an in-memory set.

**Hierarchical IDs**:
- A hierarchical ID is `parentID.N` (numeric suffix after the **last** dot).  
  This is robust to prefixes that contain dots (e.g., `my.project-abc.1`).
- Child IDs are generated via a **child_counters** table with atomic `INSERT ... ON CONFLICT` increments.
- Explicit child IDs (via import or --id) **update the counter** so future auto IDs do not collide.
- Max depth enforced by `hierarchy.max-depth` (default **3**); if unset or <1, default is used.

**Prefix extraction and validation**:
- Prefix extraction prefers the **last hyphen** when the suffix is numeric or hash-like.  
  If the suffix looks word-like (4+ chars with no digits), it falls back to the **first hyphen**.
- Explicitly provided IDs are prefix-validated **unless** `skipPrefixValidation` is set (import/multi-repo).
- Allowed prefixes are respected via `allowed_prefixes` config; **multi-repo mode allows all prefixes**.

**Partial ID resolution algorithm**:
- Fast path: try **exact ID match** via `SearchIssues` with ID filter (not `GetIssue`).
- Normalize by adding configured `issue_prefix` plus hyphen if missing (prefix default = `bd`).
- If exact match fails, scan all issues and match **hash substrings** across prefixes:
  - Prefer exact hash match (ignoring prefix), then substring match.
  - Ambiguity yields an error listing candidate IDs.

---

### 15.28 Parent Resurrection and Orphan Handling

**Orphan handling modes** (`import.orphan_handling`):
- `strict`: fail import on missing parent
- `resurrect`: attempt resurrection (default in some batch ops)
- `skip`: skip orphaned children with warning
- `allow`: allow orphans without validation (default for legacy tolerance)

**Resurrection behavior (JSONL-based)**:
- If a parent ID is missing during child ID generation or import, the system scans `.beads/issues.jsonl`
  for the **last occurrence** of that ID and creates a **closed placeholder** issue:
  - Status = `closed`, priority = 4, updated/closed timestamps = now
  - Description is prefixed with a `[RESURRECTED]` note, and the original description is appended
  - Dependencies are copied **only if the target issues already exist**
- Resurrecting a deep child will **recursively ensure ancestor chain** exists.

---

### 15.29 Update, Close, and Tombstone Semantics (Storage Layer)

**Update constraints**:
- Updates are **whitelisted** by field name; unrecognized fields are rejected.
- Status updates **cannot set `tombstone`** directly (must use delete/tombstone flow).
- `closed_at` is auto-managed when status changes unless explicitly supplied:
  - `status -> closed` sets `closed_at = now`
  - `closed -> non-closed` clears `closed_at` and **clears `close_reason`**
- Content hash is recomputed when any content fields change:
  `title`, `description`, `design`, `acceptance_criteria`, `notes`, `status`, `priority`,
  `issue_type`, `assignee`, `external_ref`.
- Updates always:
  - Create an event (`updated`, `status_changed`, `closed`, `reopened`)
  - Mark the issue as **dirty**
  - Rebuild blocked cache if `status` changed

**Close semantics**:
- `CloseIssue` sets status to `closed`, writes `closed_at`, `close_reason`,
  and `closed_by_session` (if provided).
- Close reason is stored **both** in the issue row and as the event comment.
- Close **rebuilds blocked cache** and marks issue dirty.
- Legacy code also auto-closes **convoys** (Gastown) when all tracked issues close;
  this behavior should be **excluded** in `br`.

**Tombstone semantics**:
- Tombstone creation:
  - status = `tombstone`
  - `closed_at = NULL` (enforced by constraint)
  - `deleted_at/by/reason` set, `original_type` preserved
  - event recorded, dirty marked, blocked cache rebuilt
- Hard delete:
  - Removes dependencies (both directions), events, comments, dirty markers, then issue

---

### 15.30 Ready Work and Blocked Cache (Implementation Details)

**Blocked cache**:
- `blocked_issues_cache` is a **materialized set** of blocked issue IDs.
- Rebuilt on:
  - add/remove dependency types that affect readiness
  - any status change or close (blockers change)
- Full rebuild uses recursive CTE and propagates parent-child blocking **transitively** (depth <= 50).

**Blocking rules captured in cache**:
- `blocks`: blocked while blocker is open/in_progress/blocked/deferred/hooked
- `conditional-blocks`: blocked unless blocker is closed **with failure reasons**
  (substring match of: failed, rejected, wontfix, won't fix, canceled/cancelled,
  abandoned, blocked, error, timeout, aborted)
- `waits-for`: fanout gate on children of spawner; metadata gate defaults to `all-children`
  (blocked while **any** child is open), `any-children` (blocked until **any** child closes)
- `parent-child`: children inherit parent blockage

**Ready query defaults**:
- Status defaults to `open` + `in_progress`
- Excludes `pinned` and `ephemeral` (wisps)
- Excludes internal workflow types by default: `merge-request`, `gate`, `molecule`, `message`, `agent`
- Optional filters: assignee/unassigned, labels (AND/OR), parent subtree, mol_type, priority,
  deferred handling (`include_deferred`)
- Sort policies:
  - `hybrid` (default): priority for recent items, age for older
  - `priority`
  - `oldest`

**External dependencies**:
- External dep format: `external:<project>:<capability>`
- Considered satisfied when external project has a **closed issue** with label `provides:<capability>`.
- Resolution uses **batch DB opens** per project to avoid O(N) overhead.
- Ready/blocked views filter or annotate based on external dep satisfaction.

**IsBlocked / suggest-next**:
- `IsBlocked` checks cache membership, then returns **blocking issue IDs** for `blocks` only.
- `GetNewlyUnblockedByClose` returns open/in_progress issues that depended on the closed issue
  and are **no longer in blocked cache**.

---

### 15.31 Import Pipeline Deep Dive

**Pre-processing**:
- Canonicalizes **Linear** external refs to avoid slug duplicates.
- Recomputes **content hashes** for all incoming issues (JSONL hash is not trusted).
- Marks issues as **ephemeral** if ID contains `-wisp-`.
- In multi-repo mode, **skip prefix validation** by default.
- Clears `export_hashes` before import to avoid staleness.

**Prefix mismatch handling**:
- Valid prefixes = configured prefix + `allowed_prefixes`; multi-repo allows all.
- If all mismatches are **tombstones**, they are dropped as noise.
- `--rename-on-import` rewrites prefix and **updates all text references** in:
  title, description, design, acceptance_criteria, notes, comments, dependencies.

**Match precedence and update rules**:
1) **external_ref match** (highest priority)  
2) **content hash match**  
3) **ID match** (collision/update path)
- Tombstone in DB **always blocks** resurrection.
- For updates: only apply if `incoming.updated_at` is **newer** than local.
- Timestamp-aware protection map (`ProtectLocalExportIDs`) can veto updates.
- `pinned` is only updated when explicitly true in JSONL (omitempty behavior).
- `assignee` and `external_ref` set to `NULL` if incoming value is empty.

**Batch creation**:
- New issues are sorted by **hierarchy depth** and created depth-by-depth.
- Orphan skip mode pre-filters missing-parent children to avoid partial inserts.

**Dependencies, labels, comments**:
- Dependencies are inserted if missing; FK failures are skipped with warnings (unless strict).
- Labels are additive (missing labels added, existing untouched).
- Comments are deduped by `(author, trimmed text)`; timestamps are preserved on import.

---

### 15.32 Export Error Policies and Manifests

**Error policies**:
- `strict` (default for manual export): fail fast
- `best-effort` (default for auto-export): skip with warnings
- `partial`: retry then skip with manifest entries
- `required-core`: fail on core data, skip enrichments

**Export config keys**:
- `export.error_policy`
- `auto_export.error_policy`
- `export.retry_attempts` (default 3)
- `export.retry_backoff_ms` (default 100)
- `export.skip_encoding_errors` (default false)
- `export.write_manifest` (default false)

**Manifests**:
- Written as `.manifest.json` next to JSONL using atomic temp rename.
- Permissions set to **0600** (warn-only on failure).

**Multi-repo export details**:
- Groups by `source_repo` and writes **one JSONL per repo**.
- Empty JSONL is written for repos with zero issues to keep clones in sync.
- Paths are resolved relative to **repo root** (parent of `.beads/`), not CWD.
- Uses `Lstat` for mtime updates to avoid symlink target issues.

---

### 15.33 List/Search Defaults and Filter Wiring

**List defaults**:
- If `--status` is not specified and `--all` is not set, **closed issues are excluded**.
- `--ready` forces `status=open` (not in_progress/blocked/deferred).
- Templates are excluded unless `--include-templates`.
- Gate issues are excluded unless `--include-gates` or `--type gate`.
- Directory-aware label scoping applies when **no labels are provided**.
- `--limit 0` means **unlimited**; agent mode defaults to **20** when not specified.

**Search semantics**:
- `SearchIssues` uses **LIKE** matching on `title`, `description`, and `id`.
- Default ordering is `priority ASC, created_at DESC`.
- Tombstones are excluded unless `IncludeTombstones` is set.

---

### 15.34 Update Validation Rules (Field-Level)

**Field validation rules**:
- Title length: **1..500**
- Priority: **0..4**
- IssueType: must be built-in (custom types allowed via config at higher layers)
- Estimated minutes: **non-negative**
- Status: built-in or custom; **tombstone is blocked** in updates

**Allowed update fields (storage whitelist)**:
- Core: `status`, `priority`, `title`, `description`, `design`, `acceptance_criteria`,
  `notes`, `issue_type`, `assignee`, `estimated_minutes`, `external_ref`
- Close fields: `closed_at`, `close_reason`, `closed_by_session`
- Scheduling: `due_at`, `defer_until`
- Pinned/wisp: `pinned`, `wisp` (maps to `ephemeral`)
- Additional fields exist for Gastown/agent features (exclude in `br` classic port)

---

### 15.35 Auto-Import, Auto-Flush, and Staleness Detection

**JSONL path discovery**:
- Primary resolution uses `BEADS_JSONL` env var.
- Otherwise derive `.beads/issues.jsonl` from the **DB path**; in `--no-db` mode,
  walk up to find `.beads/` and use its JSONL.
- Auto-flush creates `.beads/` if missing (best-effort).

**Auto-import trigger (hash-based, git-safe)**:
- Reads entire JSONL and computes **SHA256 hash**.
- Compares against metadata `jsonl_content_hash` (fallback: legacy `last_import_hash`).
- If hash differs → parse JSONL and import.
- Hash comparison avoids mtime pitfalls after `git pull` or file touch.

**Auto-import safety**:
- Rejects JSONL containing **git conflict markers** (`<<<<<<<`, `=======`, `>>>>>>>`) on standalone lines.
- Applies `Issue.SetDefaults()` and fixes closed-at invariant when missing.
- Marks wisps as ephemeral if ID contains `-wisp-` (defense-in-depth).
- Clears `export_hashes` before import to avoid staleness.
- Auto-import is **lenient**: `SkipPrefixValidation=true`.
- If DB is “cold-start” with no `issue_prefix`, it **infers prefix** from JSONL
  (or repo dir name) and sets config before import.

**Staleness detection (read-time)**:
- `ensureDatabaseFresh` is called by **read commands** in **direct mode**.
- It checks `autoimport.CheckStaleness`:
  - Parses `last_import_time` (RFC3339Nano, fallback RFC3339).
  - Compares JSONL **mtime** (via `Lstat`, not Stat to avoid symlink false positives).
- If stale:
  - If auto-import enabled → runs auto-import and continues.
  - If `--no-auto-import` → error with explicit recovery steps.
- `--allow-stale` skips the check with a warning.
- **Daemon mode bypass**: call sites **skip** `ensureDatabaseFresh` when daemon client is active
  because the daemon auto-imports before serving queries.

**Daemon freshness guard**:
- Long-lived daemon connections use a **FreshnessChecker** that detects DB file replacement
  (inode/mtime) and triggers a reconnection to avoid stale handles.

**Auto-flush (debounced JSONL export)**:
- All mutating commands call `markDirtyAndScheduleFlush()` or `markDirtyAndScheduleFullExport()`.
- Flush is coordinated by a **FlushManager**:
  - Single goroutine owns dirty state; no shared mutable state.
  - Debounce interval controlled by `flush-debounce` (env `BEADS_FLUSH_DEBOUNCE`).
  - `FlushNow()` bypasses debounce; `Shutdown()` performs a final flush.
- Flush is no-op if auto-flush is disabled (`--no-auto-flush`).

**Incremental vs full export**:
- Incremental: fetch dirty IDs, **merge** with existing JSONL map.
- Full: rebuild JSONL from scratch (used for ID remaps or integrity failures).
- Export always filters **wisps** and, in multi-repo mode, may filter by prefix
  for non-primary repos.
- After export:
  - Clears dirty IDs
  - Updates `jsonl_content_hash`, `jsonl_file_hash`, and `last_import_time`
    to prevent staleness loops.

**JSONL integrity validation**:
- Before flush, compare **stored JSONL file hash** with current file hash.
- On mismatch or missing JSONL:
  - Clear `export_hashes` and `jsonl_file_hash`
  - Force **full export** to restore consistency

---

### 15.36 Delete/Tombstone Workflow and Reference Rewriting

**Delete command flow (direct mode)**:
1) Preview unless `--force`  
2) Update connected issues’ text references: replace `ID` → `[deleted:ID]`  
3) Remove dependency links (both directions)  
4) Create tombstone (default) or hard delete if requested  
5) Mark dirty + auto-flush

**Reference rewriting (edge-safe)**:
- Uses a boundary-aware regex: ID is replaced only when surrounded by **non-word**
  boundaries where “word” includes letters, digits, `_`, and `-`.
- Fields updated: `description`, `notes`, `design`, `acceptance_criteria`.
- The in-memory copies of connected issues are updated after replacement to avoid double-rewrite.

**Batch deletion**:
- Uses `DeleteIssues` (storage) for cascade/force semantics and returns:
  - `deleted_count`, `dependencies_removed`, `labels_removed`, `events_removed`
  - `orphaned_issues` (when forced without cascade)
- Connected issues are pre-collected and updated after deletion with the same reference rewrite logic.

**Cascade vs force**:
- `--cascade`: recursively deletes all dependents.
- `--force`: deletes the selected issues and **orphans** dependents.
- Without either: deletion fails if dependents exist (preview explains).

**Hard delete semantics**:
- `--hard` **does not remove tombstones from DB**; it **prunes tombstones from JSONL**
  immediately (negative TTL), then advises `bd cleanup --hard` after sync.
- Tombstones remain in DB to prevent resurrection by subsequent imports.

**Tombstone pruning (JSONL-only)**:
- `pruneExpiredTombstones` reads JSONL, removes tombstones that exceeded TTL:
  - Default TTL = `DefaultTombstoneTTL` (30 days).
  - Custom TTL overrides; negative TTL means prune immediately.
- Writes JSONL atomically and reports pruned IDs/count.

---

### 15.37 Routing and Multi-Repo Resolution (routes.jsonl)

**Two routing planes**:
- **Creation routing** (where new issues go): controlled by `routing.mode`
  (`auto` vs `explicit`) and `routing.default/maintainer/contributor`.
  - Role detection: git config `beads.role` or push URL heuristics.
  - `--repo` overrides everything.
- **ID routing** (where existing issues live): driven by `routes.jsonl` + redirect.

**routes.jsonl format**:
- JSONL entries: `{ "prefix": "bd-", "path": "beads/mayor/rig" }`
- Comments and blank lines are ignored.
- Prefix includes the trailing hyphen.

**Lookup resolution**:
- For a given ID, extract prefix (substring before first `-`, plus hyphen).
- Search order:
  1) `routes.jsonl` in the **current** `.beads/`
  2) If not found, walk up to **town root** (identified by `mayor/town.json`) and use
     `<townRoot>/.beads/routes.jsonl`
- If a prefix match is found:
  - `path == "."` → town-level `.beads/`
  - Otherwise resolve relative to town root: `<townRoot>/<path>/.beads`
  - Apply **redirect** (see below)
  - Validate target directory exists before routing

**Redirect file**:
- `.beads/redirect` may contain a path to another beads directory.
- If present, routing follows the redirect (relative paths resolved from the current beads dir).
- Used for local overrides, migrations, or multi-worktree setups.

**Forgiving rig lookup** (`--rig` / `--prefix` UX):
- Accepts `rig`, `prefix` (`bd`), or `prefix-` (`bd-`).
- Uses routes + town root to resolve to target beads dir and prefix.

**External ref derivation**:
- If an ID prefix matches a route, it can be converted to
  `external:<project>:<id>` where `<project>` is the first path segment.

**Routed storage access**:
- When an ID is routed, the CLI opens a **separate SQLite connection** to the routed beads dir.
- Callers must close routed storage after use.

---

### 15.38 Initialization and Bootstrap (bd init)

**Non-invasive port note**:
- Legacy bd installs git hooks, merge drivers, and writes repo-specific git config in init flows.
- `br` must **not** auto-install or auto-modify git state. Any git integration must be **explicit**
  and opt-in (per project goals).

**Purpose**:
- Creates the local `.beads/` workspace and initializes storage.
- Seeds the database from JSONL in git history or local JSONL when requested.

**Backend and path selection**:
- `--backend` accepts `sqlite` (default) and `dolt` (legacy; not ported to `br`).
- Database path comes from `--db`, else `BEADS_DB`, else `.beads/beads.db`.
- If DB path is inside `.beads/`, init also creates `.beads/` and local files.

**Prefix resolution** (in order):
1) `--prefix` flag  
2) config `issue-prefix` (from config.yaml or env)  
3) first issue found in JSONL from git history  
4) current directory name  
- Trailing `-` is stripped (hyphen is added during ID generation).

**Safety guards**:
- Refuses to run inside a git worktree; must initialize in the main repo root.
- Refuses to run inside `.beads/` to avoid nested beads directories.
- Checks for an existing database; aborts unless `--force`.
- If JSONL exists but DB does not, init proceeds (fresh-clone recovery path).
- Migrates older `*.db` filenames to `beads.db` if exactly one DB exists.

**No-DB mode (`--no-db`)**:
- Creates `.beads/issues.jsonl` and `.beads/interactions.jsonl`.
- Writes `metadata.json` (configfile defaults) and `.beads/config.yaml`.
- Stores `issue-prefix` in config.yaml because there is no DB config table.

**SQLite mode (default)**:
- Creates/updates `.beads/.gitignore` from doctor template.
- Ensures `.beads/interactions.jsonl` exists.
- Creates storage backend and sets DB config `issue_prefix` (fatal if failure).
- Writes metadata keys: `bd_version`, `repo_id`, `clone_id` (best-effort).
- Writes/updates `.beads/metadata.json` and `.beads/config.yaml` templates.

**Sync-branch setup**:
- `--branch` sets sync branch (writes both config.yaml and DB).
- No auto-detection of current branch to avoid main/master worktree conflicts.

**Import on init**:
- Default: scan git history for `.beads/issues.jsonl`, import if found.
- `--from-jsonl`: import from local `.beads/issues.jsonl` (preserves manual cleanups).
- Auto-import failure is non-fatal; init continues with empty DB.

**Optional wizards and git setup**:
- `--contributor` / `--team` run guided setup wizards.
- `--setup-exclude` configures `.git/info/exclude` for forks.
- Installs git hooks and merge driver unless `--skip-hooks` / `--skip-merge-driver`.
- `--stealth` mode configures per-repo git excludes and suppresses hooks/driver.

**Post-init diagnostics**:
- Adds "landing the plane" instructions to `AGENTS.md` and `@AGENTS.md` (unless stealth).
- Runs `bd doctor` checks; warns if setup is incomplete.

---

### 15.39 Sync Semantics (bd sync, classic SQLite + JSONL)

**Non-invasive port note**:
- Legacy bd’s sync assumes automatic git commit/push behavior in many flows.
- `br` must be **less invasive**: no automatic hooks, no background daemon actions,
  no implicit commit/push. Any git ops must be explicit CLI commands.

**Direct-mode enforcement**:
- If daemon is connected, `bd sync` forces direct mode to avoid stale DB handles.
- Store initialization is re-run after daemon disconnect.

**Modes and early exits**:
- `--import-only`: import JSONL only (no git ops; inline import to handle redirects).
- `--flush-only`: export JSONL only.
- `--squash`: export JSONL only, skipping commit/push (accumulate changes).
- `--status`: show sync-branch vs main diff.
- `--merge`: merge sync-branch back to main.
- `--check`: integrity checks (force-push, prefix mismatch, orphaned children).
- `--from-main`: one-way sync from default branch for ephemeral branches.

**Redirect handling**:
- If `.beads/redirect` is active, sync skips all git operations and only exports.
- Redirect and sync-branch are mutually exclusive; sync warns and proceeds export-only.

**Preflight safety**:
- Requires a git repo (unless using export-only modes).
- Aborts if merge/rebase in progress.
- Detects uncommitted JSONL changes; re-exports from DB to reconcile.
- If no upstream and no sync-branch, auto-falls back to `--from-main`.

**Pull-first sync (default)**:
1) Load local issues from DB (includes tombstones).
2) Acquire `.beads/.sync.lock` (exclusive).
3) Load base state from `.beads/sync_base.jsonl` (may be missing on first sync).
4) Pull remote JSONL (sync-branch worktree if configured, else normal pull).
5) Load remote JSONL state.
6) 3-way merge: base + local + remote (LWW + union/append rules).
7) Write merged JSONL, import merged state into DB.
8) Export DB back to JSONL (DB is source of truth).
9) Commit/push changes (sync-branch worktree or current branch).
10) Save new base state and clear sync state.

**No-pull sync (`--no-pull`)**:
- Export -> commit -> push, under the same lock.
- Runs pre-export validation, duplicate ID checks, and orphaned dep warnings.

**From-main mode**:
- Uses `sync.remote` (default `origin`) and detects default branch.
- `git fetch <remote> <main>` then checks out `.beads/` from remote main.
- Imports JSONL into DB; forces `no-git-history` to avoid false deletions.

**Key flags**:
- `--rename-on-import`: rewrite IDs to local prefix and update references.
- `--no-git-history`: skip git history backfill for deletions (useful for JSONL renames).
- `--accept-rebase`: accept remote sync-branch history resets (integrity flow).

**Integrity check output**:
- Reports force-push risk, prefix mismatches, and orphaned children.
- Exits non-zero if any problems are detected.

---

### 15.40 Configuration System (config.yaml + DB config)

**Non-invasive port note**:
- `br` should avoid hidden or implicit config that triggers git ops or background behavior.
- Keep config explicit; prefer on-demand commands over daemon-autostart defaults.

**Config sources and precedence**:
- Load order: project `.beads/config.yaml` (walk up from CWD), then
  `~/.config/bd/config.yaml`, then `~/.beads/config.yaml`.
- Environment variables override config file (prefix `BD_`, dot/hyphen -> underscore).
- Selected legacy env vars are also bound: `BEADS_FLUSH_DEBOUNCE`,
  `BEADS_AUTO_START_DAEMON`, `BEADS_IDENTITY`, `BEADS_REMOTE_SYNC_INTERVAL`.
- Flags override config values at command execution time.

**Two storage planes**:
- **YAML-only**: startup settings read before DB exists (must live in config.yaml).
- **DB config**: runtime settings stored in `config` table (versioned with DB).

**YAML-only keys (representative)**:
- Bootstrap: `no-db`, `no-daemon`, `no-auto-flush`, `no-auto-import`, `json`,
  `auto-start-daemon`
- Identity/paths: `db`, `actor`, `identity`
- Timing: `flush-debounce`, `lock-timeout`, `remote-sync-interval`
- Git: `git.author`, `git.no-gpg-sign`, `no-push`, `no-git-ops`
- Sync: `sync-branch`, `sync.branch` (alias), `sync.require_confirmation_on_mass_delete`
- Routing: `routing.*`
- Create/validation: `create.require-description`, `validation.*`
- Hierarchy: `hierarchy.max-depth`
- Daemon: `daemon.*`
- Any key with prefixes `routing.`, `sync.`, `git.`, `directory.`, `repos.`,
  `external_projects.`, `validation.`, `daemon.`, `hierarchy.` is YAML-only.

**Config set/get/list/unset behavior**:
- `bd config set <key> <value>`:
  - YAML-only keys update config.yaml (uncomment or append), preserving file.
  - DB keys require direct mode; `sync.branch` uses sync-branch helper.
- `bd config get <key>`:
  - YAML-only keys read from config.yaml (empty => "not set in config.yaml").
  - DB keys require direct mode; `sync.branch` resolves env/yaml/db precedence.
- `bd config list`:
  - Lists DB config only, sorted.
  - Warns when config.yaml or env overrides DB values (notably sync.branch).
- `bd config unset <key>`:
  - Deletes DB config entry only (does not edit config.yaml).

**YAML editing rules**:
- Values are formatted: booleans lowercased, numbers/durations unquoted,
  other strings quoted.
- `sync.branch` is normalized to `sync-branch` in YAML.
- `hierarchy.max-depth` is validated to be an integer >= 1.

**Sync-branch precedence**:
1) `BEADS_SYNC_BRANCH` env var  
2) `sync-branch` in config.yaml  
3) `sync.branch` in DB  
- `sync.branch` cannot be `main` or `master` (worktree conflict prevention).

---

### 15.41 Output Formats and JSON Schemas (Core Commands)

**Global JSON conventions**:
- All JSON uses stable field names from `types.*` structs (snake_case).
- Arrays are always emitted as `[]` (never `null`) in JSON mode.
- Time fields serialize as RFC3339 strings (Go `time.Time` JSON).
- `omitempty` fields are omitted when empty.
- JSON is pretty-printed with 2-space indent in core CLI (`outputJSON`).
- JSON errors (when `--json` is active) are emitted to stderr as:
  `{ "error": "...", "code": "..." }` (code may be omitted).

#### 15.41.1 list

**JSON shape**: array of `IssueWithCounts`
```json
[
  {
    "id": "bd-abc123",
    "title": "Example",
    "status": "open",
    "priority": 2,
    "issue_type": "task",
    "labels": ["backend"],
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-02T00:00:00Z",
    "dependency_count": 1,
    "dependent_count": 3
  }
]
```
**Text formats**:
- **Compact (default)**: one line per issue  
  `STATUS_ICON [pin] ID [P#] [type] @assignee [labels] - Title`
  - Closed issues are fully muted.
  - Priority uses `● Pn` (colored for P0/P1).
- **Long (`--long`)**: multi-line per issue:  
  `ID [P#] [type] status` then title, optional assignee/labels.
- **Pretty/Tree (`--pretty` or `--tree`)**:
  - Tree connectors `├──`, `└──` with priority-sorted children.
  - Status icons: `○` open, `◐` in_progress, `●` blocked, `✓` closed, `❄` deferred.
  - Root list ends with summary and status key.
- **Agent mode**: `ID: Title` only (no colors, no emojis).
- **Format (`--format`)**:
  - `dot` → Graphviz DOT graph with colored nodes and dependency edges.
  - `digraph` → edge list (issue_id depends_on_id) for `digraph` tool.
  - Any other string is treated as a Go template executed per dependency edge.
- **Pager**: non-agent outputs are piped through a pager unless `--no-pager`.

#### 15.41.2 show

**JSON shape**: array of `IssueDetails` (even for a single ID)
```json
[
  {
    "id": "bd-abc123",
    "title": "Example",
    "status": "in_progress",
    "priority": 1,
    "issue_type": "feature",
    "labels": ["ui", "frontend"],
    "dependencies": [
      { "id": "bd-xyz", "title": "Blocker", "dependency_type": "blocks" }
    ],
    "dependents": [
      { "id": "bd-foo", "title": "Child", "dependency_type": "parent-child" }
    ],
    "comments": [
      { "id": 3, "issue_id": "bd-abc123", "author": "alice", "text": "...", "created_at": "..." }
    ],
    "parent": "bd-xyz"
  }
]
```
**Text formats**:
- **Short (`--short`)**: same line format as pretty list (`STATUS_ICON ID ●P# [type] Title`).
- **Full**:
  - Tufte header: `STATUS_ICON ID · Title   [● P# · STATUS]`
  - Metadata lines: owner/assignee/type and created/updated (plus due/defer if set).
  - Sections (if present): DESCRIPTION / DESIGN / NOTES / ACCEPTANCE CRITERIA (markdown rendered).
  - Labels, dependencies, dependents grouped by type, and comments with date+author.

#### 15.41.3 ready

**JSON shape**: array of `Issue`
```json
[
  { "id": "bd-abc", "title": "Ready item", "status": "open", "priority": 2, "issue_type": "task" }
]
```
**Text**:
- Default: numbered list with `[P#] [type] ID: Title`, plus estimate/assignee if present.
- `--pretty` uses the same tree output as `list --pretty`.
- If none: prints either “No ready work found…” or “No open issues”.

#### 15.41.4 blocked

**JSON shape**: array of `BlockedIssue`
```json
[
  {
    "id": "bd-abc",
    "title": "Blocked item",
    "status": "blocked",
    "priority": 1,
    "blocked_by_count": 2,
    "blocked_by": ["bd-1", "bd-2"]
  }
]
```
**Text**:
- `[P#] ID: Title` then `Blocked by N open dependencies: [ids]`.

#### 15.41.5 search

**JSON shape**: array of `IssueWithCounts` (same as list).

**Text**:
- Header: `Found N issues matching 'query'`
- **Compact (default)**: `ID [P#] [type] status @assignee [labels] - Title`
- **Long (`--long`)**: multi-line with assignee + labels.
- **Sorting**: optional `--sort` and `--reverse` are applied after search results load.

#### 15.41.6 status (alias: stats)

**JSON shape**: `StatusOutput`
```json
{
  "summary": {
    "total_issues": 42,
    "open_issues": 10,
    "in_progress_issues": 5,
    "closed_issues": 20,
    "blocked_issues": 2,
    "deferred_issues": 1,
    "ready_issues": 7,
    "tombstone_issues": 3,
    "pinned_issues": 1,
    "epics_eligible_for_closure": 1,
    "average_lead_time_hours": 12.5
  },
  "recent_activity": {
    "hours_tracked": 24,
    "commit_count": 3,
    "issues_created": 2,
    "issues_closed": 1,
    "issues_updated": 4,
    "issues_reopened": 0,
    "total_changes": 7
  }
}
```
**Text**:
- “Issue Database Status” header with summary and optional extended section.
- Optional “Recent Activity (last 24 hours)” block.
- `bd stats` is a direct alias of `bd status`.

#### 15.41.7 count

**JSON (no grouping)**:
```json
{ "count": 17 }
```
**JSON (grouped)**:
```json
{
  "total": 17,
  "groups": [
    { "group": "open", "count": 5 },
    { "group": "closed", "count": 12 }
  ]
}
```
**Text**:
- No grouping: prints a single number.
- Grouped: `Total: N` then `group: count` sorted by group key.

#### 15.41.8 stale

**JSON shape**: array of `Issue`
```json
[ { "id": "bd-abc", "title": "Stale", "updated_at": "..." } ]
```
**Text**:
- “Stale issues (N not updated in D+ days):” with numbered list.
- Each item prints status, days stale, and optional assignee.

#### 15.41.9 dep

**dep add / dep --blocks JSON**:
```json
{ "status": "added", "issue_id": "bd-a", "depends_on_id": "bd-b", "type": "blocks" }
```
or (for `--blocks`):
```json
{ "status": "added", "blocker_id": "bd-b", "blocked_id": "bd-a", "type": "blocks" }
```
**dep remove JSON**:
```json
{ "status": "removed", "issue_id": "bd-a", "depends_on_id": "bd-b" }
```
**dep list JSON**: array of `IssueWithDependencyMetadata`  
(all Issue fields + `dependency_type`).

**dep tree JSON**: array of `TreeNode`
```json
[
  { "id": "bd-root", "depth": 0, "parent_id": "", "truncated": false, "title": "Root" },
  { "id": "bd-child", "depth": 1, "parent_id": "bd-root", "truncated": false, "title": "Child" }
]
```
**dep cycles JSON**: array of cycles, each a list of Issue objects.

**Text**:
- `dep list`: “ID: Title [P#] (status) via <dependency_type>”
- `dep tree`: tree connectors with status emoji and optional mermaid output.
- `dep cycles`: numbered cycles with issue IDs + titles.

#### 15.41.10 label

**label add/remove JSON**: array of results
```json
[ { "status": "added", "issue_id": "bd-1", "label": "backend" } ]
```
**label list JSON**: array of strings
```json
["backend", "ui"]
```
**label list-all JSON**: array of `{label,count}` objects.

**Text**:
- Add/remove prints per-issue confirmation.
- List prints bullet list; list-all prints aligned label table with counts.

#### 15.41.11 comments

**comments list JSON**: array of `Comment`
```json
[ { "id": 1, "issue_id": "bd-1", "author": "alice", "text": "...", "created_at": "..." } ]
```
**comments add JSON**: a single `Comment` object.

**Text**:
- List: “Comments on <id>” then `[author] at YYYY-MM-DD HH:MM` followed by
  markdown-rendered comment text indented.
- Add: “Comment added to <id>”.

---

### 15.42 List/Search Filtering Semantics (Classic Port Requirements)

**Limit defaults**:
- `list`: default 50; if `--limit` is explicitly set to `0`, means unlimited.
- `list` in agent mode: default 20 (only when `--limit` not specified).
- `search`: default 50; no agent-mode override.
- `ready`: default 10.
- `stale`: default 50.

**Default status filtering**:
- `list`: if `--status` not set and `--all` not set, **exclude closed**.
- `list --ready`: forces `status=open` only (no in_progress/blocked/deferred).
- `search`: no implicit exclusion; includes closed unless `--status` is specified.
- `search` query matches title/description/id (OR semantics).

**Label filter semantics**:
- `--label` = AND (issue must have all labels).
- `--label-any` = OR (issue must have at least one).
- If **no labels specified**, `list` and `ready` apply `directory.labels`
  from config for monorepo scoping; `search` does **not**.

**Ordering defaults**:
- Storage query default ordering is `priority ASC, created_at DESC`.
- `--sort` overrides with in-memory ordering:
  - `priority` ascending (P0 before P1).
  - `created`, `updated`, `closed` are descending (newest first).
  - `status`, `id`, `title`, `type`, `assignee` are ascending.
  - `--reverse` flips the chosen sort.

**Pretty/tree fallbacks**:
- `--tree` is an alias for `--pretty`.
- `--watch` implies `--pretty`.
- Tree building uses parent-child dependencies; if not available, falls back
  to dotted hierarchical IDs (`parent.1`, `parent.1.2`).

**Core JSON shapes (for spec)**:
- `IssueWithCounts`: flattened Issue fields + `dependency_count`, `dependent_count`.
- `IssueDetails`: flattened Issue fields + `labels`, `dependencies`, `dependents`,
  `comments`, `parent` (computed from parent-child deps).

---

### 15.43 Dependency Tree Output (bd dep tree)

**Tree node schema** (`TreeNode`):
- All `Issue` fields (ID, title, status, priority, etc.).
- `depth` (int): edges from root (root = 0).
- `parent_id` (string): immediate parent in the traversal.
  - Root node uses `parent_id = id`.
- `truncated` (bool): true if `depth == maxDepth`.

**Traversal direction**:
- **Down (default)**: dependencies (“what blocks this issue”).
  - Parent → child edges follow: **issue depends on dependency**.
- **Up (`--direction=up`)**: dependents (“what this issue blocks”).
  - Parent → child edges follow: **dependent depends on parent**.
- **Both**: current implementation concatenates “up” nodes and “down” nodes;
  visual separation is minimal (legacy behavior).

**Depth ordering & deterministic output**:
- Recursive CTE yields rows ordered by `depth ASC, priority ASC, id ASC`.
- This ordering drives the initial `TreeNode` list and the child insertion order
  in the renderer (stable, deterministic).

**Parent pointer logic**:
- Each recursion step sets `parent_id = t.id` (the node from which we traversed).
- For **down**: child is the dependency target.
- For **up**: child is the dependent.
- The renderer builds a `parent_id -> children[]` map to render connectors.

**Truncation rules**:
- `maxDepth <= 0` defaults to **50**.
- Nodes at `depth == maxDepth` are flagged `truncated=true`.
- Renderer appends an ellipsis (`…`) when:
  - `node.truncated` is true, **or**
  - `depth == maxDepth` and the node has children (indicates hidden depth).

**Cycle protection**:
- Recursive query maintains a path string with `→` separators and excludes
  revisits of any ID already on the path.

**Deduplication vs “show all paths”**:
- `--show-all-paths=false` (default):
  - Storage **dedupes by ID**, keeping only the shallowest occurrence
    (first row due to depth ordering).
- `--show-all-paths=true`:
  - Storage emits **all path occurrences** (diamonds preserved).
  - Renderer still prints a short “(shown above)” line for already-seen IDs,
    to avoid infinite repeats in display.

**External leaf node synthesis**:
- Only in **down** direction (dependencies).
- External dependencies are stored as `external:<project>:<capability>`.
- Synthetic `TreeNode` is created per external ref:
  - `id` = full external ref
  - `parent_id` = issue that depends on the external ref
  - `depth` = parent depth + 1
  - `status` = `closed` if satisfied, otherwise `blocked`
  - `title` = `"✓ <capability>"` (satisfied) or `"⏳ <capability>"` (unsatisfied)
  - `priority` = 0, `issue_type` = task
- In text rendering, external nodes print as:  
  `✓ capability (external)` or `⏳ capability (external)` (with status color).

**Text rendering details**:
- Uses tree connectors (`├──`, `└──`) and vertical bars for siblings.
- Each line is: `ID: Title [P#] (status)` plus `[READY]` on root if open.
- External refs are printed from `title` only (ID hidden) and suffixed `(external)`.

---

### 15.44 Comments Command Behavior (bd comments)

**List (`bd comments <id>`)**:
- Resolves partial IDs to full IDs (daemon: RPC resolve; direct: local resolve).
- Loads comments ordered by `created_at ASC`.
- JSON output: array of `Comment` objects (pretty printed):
```json
[
  { "id": 1, "issue_id": "bd-1", "author": "alice", "text": "...", "created_at": "..." }
]
```
- Text output:
  - Header `Comments on <id>`
  - For each comment: `[author] at YYYY-MM-DD HH:MM`
  - Comment body is markdown-rendered and indented.

**Add (`bd comments add <id> <text>` or `-f <file>`)**:
- Resolves partial ID (same as list).
- Author resolution (if `--author` not set):
  - `--actor` → `BD_ACTOR` → `BEADS_ACTOR` → `git config user.name` → `$USER` → `"unknown"`.
- Inserts a new row with `created_at = CURRENT_TIMESTAMP`.
- **No deduplication**: repeated identical text is allowed.
- JSON output: a single `Comment` object (pretty printed).
- Empty text is allowed (explicitly tested).

**Dedupe rules (import only)**:
- Comment dedupe happens **during JSONL import**, not during CLI add.
- Import path dedupes by `(author, trimmed text)` to avoid duplicate comments.

---

### 15.45 dep + label Command UX (Daemon vs Direct)

#### dep (dependencies)

**Add (dep add / dep --blocks)**:
- Accepts positional or flag syntax:
  - `bd dep add <issue> <depends-on>`
  - `bd dep <blocker> --blocks <blocked>`
- Partial IDs are resolved before mutation:
  - **Daemon**: RPC `ResolveID`.
  - **Direct**: local resolver.
- `external:<project>:<capability>` is allowed as the dependency target:
  - Validated to `external:<project>:<capability>` format.
  - Stored **as-is** (no ID resolution).
- **Anti-pattern guard**: child → parent dependency is rejected to prevent deadlocks.
- **Cycle warning**: after adding, a cycle scan runs and emits a warning.

**Remove (dep remove)**:
- Requires two IDs (issue + depends-on), resolves partial IDs first.
- **Daemon**: RPC `RemoveDependency`.
- **Direct**: `RemoveDependency` + mark dirty for auto-flush.

**List (dep list)**:
- Default direction = down (“depends on”); `--direction=up` shows dependents.
- Optional `--type` filters by dependency type.
- **Daemon**: list uses local DB if RPC unsupported (fallback to direct mode).

**Tree (dep tree)**:
- Uses `GetDependencyTree` (see §15.43).
- `--direction` overrides deprecated `--reverse`.
- `--format=mermaid` outputs flowchart TD.
- **Daemon**: if RPC unsupported, falls back to direct store.

**Error handling differences**:
- **Daemon mode** returns RPC errors verbatim; unknown ops trigger direct fallback.
- **Direct mode** returns storage errors; CLI wraps with `FatalErrorRespectJSON`
  to emit JSON error objects when `--json` is active.

#### label

**Add / remove**:
- Batch operations: `label add <issue...> <label>`, `label remove <issue...> <label>`.
- Partial IDs are resolved first (daemon or direct).
- `provides:*` labels are **reserved**; only `bd ship` may add them.
- **Daemon**: RPC `AddLabel` / `RemoveLabel`.
- **Direct**: `AddLabel` / `RemoveLabel` + mark dirty for auto-flush.

**List**:
- `label list <issue>` returns labels for a single issue.
- `label list-all` aggregates unique labels with counts.

**Error handling differences**:
- In daemon mode, label list relies on RPC `Show` and reads labels from response.
- Direct mode queries label tables and returns missing labels as empty list.
- JSON output is always an array (labels or `{label,count}` objects).

---

### 15.46 Doctor / Integrity Checks (Classic Subset)

**Port decision (v1, non-invasive)**:
- Keep **read-only diagnostics** that help detect corruption or drift.
- **Do NOT** port any auto-fix behaviors that delete, rename, or modify git state.
- Suggested recovery steps may be printed, but must be **explicit user actions**.

**JSONL integrity**:
- Locates JSONL via `metadata.json` (configfile JSONLExport) or best-effort discovery.
- Scans line-by-line; malformed JSON or missing `id` is counted and reported.
- If malformed and DB exists: warning/error with guidance to regenerate JSONL.
- If malformed and DB missing: error with guidance to restore JSONL from git/backups.

**Schema compatibility**:
- Opens DB read-only and probes critical tables/columns:
  - `issues`, `dependencies`, `child_counters`, `export_hashes`.
- Missing tables/columns → error with migration guidance.

**Database integrity (SQLite PRAGMA)**:
- Runs `PRAGMA integrity_check` on DB.
- “ok” → pass; any other result → corruption warning.
- If JSONL is present, suggests recovery via import/export (no auto-fix).

**DB ↔ JSONL sync check**:
- Counts issues in DB and JSONL and compares.
- Detects prefix mismatch (majority-prefix rule; allows `-mol/-wisp/-eph` variants).
- Detects status mismatches when counts match (DB vs JSONL).
- Uses file mtime as a secondary stale signal (warn if JSONL much newer).

**Orphaned dependencies** (classic validation):
- SQL join to detect dependencies whose `depends_on_id` does not exist in `issues`.
- Report count + sample list.
- **No auto-removal in v1**; recommend manual cleanup or explicit repair command.

**Merge artifacts**:
- Scans `.beads/` for merge temp files (`*.base.jsonl`, `*.left.jsonl`, etc.).
- Reports and suggests manual cleanup.

**Excluded in v1**:
- `doctor --fix` automation (JSONL regeneration, pruning, dependency cleanup).
- Git operations (hook fixes, sync-branch resets, branch manipulations).
- Any destructive recovery that renames or deletes files.

---

### 15.47 Create / Update / Close / Reopen (Mutation Semantics)

This section documents the CLI-visible semantics for classic issue mutation commands.
Gastown-only flags and types are noted but excluded from the Rust port (see Port Notes).

#### 15.47.1 create

**Invocation**:
- `bd create <title>` or `bd create --title <title>` (positional and flag must match if both present).
- `bd create --file <markdown>` creates multiple issues parsed from a Markdown file.
  - `--dry-run` is not supported with `--file`.

**Defaults & validation**:
- Default `type`: `task` (flag default).
- Default `priority`: `2` (P2).
- Status is always `open` on create.
- `create.require-description` in `config.yaml` may enforce description.
  - If missing description and not forced, emits warning unless `--silent`.
- `--priority` accepts `0-4` or `P0-P4` (P0 is highest).
- `--type` validates against built-in types (plus custom types if configured).

**IDs & hierarchy**:
- `--id` sets explicit ID; `--parent` cannot be used simultaneously.
- If `--parent` is provided in direct mode, child ID is generated by
  `GetNextChildID(parent)`, yielding hierarchical IDs (`bd-abc.1`, `bd-abc.2`, ...).
- Explicit IDs are validated:
  - Prefix format and length
  - Prefix must match `issue_prefix` or `allowed_prefixes` unless `--force`
  - Agent IDs (if `--type=agent`) have additional validation (Gastown-only).

**Dependencies & labels**:
- `--deps` supports `type:id` or `id` (default type `blocks`).
- `--waits-for` and `--waits-for-gate` attach a waits-for dependency with metadata.
- `--parent` also creates a `parent-child` dependency after create.
- Labels are added after creation; failures are warnings, not fatal.

**Routing flags (classic + Gastown)**:
- `--repo` bypasses auto-routing and creates in that repo (direct mode only).
- `--rig` / `--prefix` route to a different rig via `routes.jsonl` (Gastown).
  - See §15.54 for details.

**Output**:
- `--json`: prints the created issue object (full Issue schema).
- `--silent`: prints only the ID (no extra text).
- Default: human-readable summary:
  - `Created issue: <id>`
  - `Title`, `Priority`, `Status` lines.

#### 15.47.2 update

**Invocation**:
- `bd update <id...>` updates one or more issues.
- If no ID is provided, uses `last_touched` (most recent create/update/show/close).

**Field updates**:
- Supports updates to `status`, `priority`, `title`, `description`, `design`,
  `acceptance_criteria`, `notes`, `assignee`, `external_ref`, `estimate`, `type`.
- Label operations:
  - `--add-label`, `--remove-label`, `--set-labels`.
- Parent reparenting via `--parent`.
- `--due` / `--defer` accept relative or absolute time parsing and can be cleared.

**Claim behavior**:
- `--claim` performs an atomic claim:
  - Fails if already assigned
  - Sets `assignee = actor` and `status = in_progress`.

**Status close shortcut**:
- If status set to `closed`, include `closed_by_session` if
  `--session` or `CLAUDE_SESSION_ID` is available.

**Output**:
- `--json`: array of updated Issue objects (one per ID updated).
- Human: per-issue confirmation or warnings.

#### 15.47.3 close

**Invocation**:
- `bd close <id...>` or closes `last_touched` if no ID supplied.

**Core behavior**:
- Validates closability (pinned requires `--force`).
- Blocks closure if issue is blocked by open blockers (unless `--force`).
- Sets status to `closed`, `closed_at`, and `close_reason` (from `--reason`).

**Special modes**:
- `--suggest-next`: includes newly unblocked issues (direct mode only).
- `--continue`: in molecule workflows, auto-advance to next step (Gastown).

**Output**:
- `--json` default: array of closed issues.
- If `--suggest-next`: JSON object `{ "closed": [...], "unblocked": [...] }`.
- If `--continue`: JSON object `{ "closed": [...], "continue": { ... } }`.
- Human output: `Closed <id>: <reason>` per issue.

#### 15.47.4 reopen

**Behavior**:
- Explicit command to reopen closed issues (sets `status=open` and clears `closed_at`).
- Adds an optional comment if `--reason` is provided.
- Uses `ResolvePartialID` in direct mode, RPC in daemon mode.

**Output**:
- `--json`: array of reopened Issue objects.
- Human: `Reopened <id>` with optional `: <reason>`.

**Port notes (non-invasive)**:
- `br` should preserve the above semantics for classic use.
- Gastown-only flags (mol, agent, gate, rig, convoy) are excluded from v1.

---

### 15.48 Import / Export (JSONL)

#### 15.48.1 import

**Input**:
- Reads JSONL from stdin or `-i <file>`.
- Rejects positional arguments with a helpful hint.

**Direct-mode only**:
- Import bypasses daemon; uses direct SQLite connection.

**Conflict handling**:
- Detects git conflict markers in JSONL and attempts automatic 3-way merge.
- On merge success, restarts import from merged file.

**Initialization**:
- If DB exists but `issue_prefix` missing, detects prefix from JSONL.
- Falls back to directory name if JSONL has no detectable prefix.

**Collision and prefix mismatch**:
- Prefix mismatches produce a detailed report; can be fixed with:
  - `--rename-on-import` (remap IDs to DB prefix)
  - or later `bd rename-prefix`.
- Collisions (same ID, different content) abort with report.

**Flags (classic)**:
- `--skip-existing`: create-only (do not update existing).
- `--strict`: fail on any errors in dependencies/labels.
- `--rename-on-import`: rename imported IDs to configured prefix.
- `--orphan-handling`: `strict|resurrect|skip|allow`.
- `--clear-duplicate-external-refs`: clears duplicates rather than error.
- `--dedupe-after`: post-import duplicate detection summary.
- `--dry-run`: preview counts, do not write.
- `--force`: update import metadata even if no changes.

**Output**:
- No structured JSON output (even when `--json` is set).
- Human output goes to stderr.
- Summary includes counts: created, updated, unchanged, skipped, remapped.

#### 15.48.2 export

**Formats**:
- `jsonl` (default): one JSON object per line.
- `obsidian`: Markdown task format (default output `ai_docs/changes-log.md`).

**Filters**:
- `--status`, `--assignee`, `--type`, `--label`, `--label-any`.
- `--priority`, `--priority-min`, `--priority-max`.
- `--created-*`, `--updated-*` date filters.

**Tombstones and wisps**:
- Tombstones included by default (if no status filter), to propagate deletes.
- Ephemeral/wisp issues are always excluded from JSONL export.

**Safety checks**:
- Refuses to export if DB has fewer issues than existing JSONL (staleness guard).
- Can override with `--force`.

**Output**:
- JSONL writes to stdout or file; order is sorted by ID.
- If `--json` is set: outputs a stats object to stderr, not stdout:
  - `{ success, exported, skipped, total_issues, output_file? }`.

---

### 15.49 Sync / Flush / Auto-Flush (Classic vs Non-Invasive Port)

**Legacy behavior**:
- `bd sync` performs git pull/merge, 3-way merge, export, commit, push.
- `--flush-only` exports pending changes to JSONL (no git operations).
- `--import-only` imports from JSONL (no git operations).
- Auto-flush manager debounces writes and exports JSONL after changes.
- Auto-import triggers when JSONL content hash differs from DB metadata.

**Classic port intent (br)**:
- Keep explicit `export` and `import` commands.
- Keep an explicit `sync --flush-only` alias for export (no git operations).
- Drop automatic git operations, pull/push, auto-commit, and sync-branch.
- Disable by default any auto-import/auto-flush that changes files without
  explicit user action (must be opt-in if retained at all).

---

### 15.50 Maintenance Commands (classic subset)

#### 15.50.1 cleanup

**Purpose**:
- Convert closed issues to tombstones and prune expired tombstones.

**Safety**:
- Requires `--force` unless `--dry-run`.
- `--hard` bypasses tombstone TTL safety (prunes immediately).
- `--cascade` deletes dependents recursively.
- `--older-than N` restricts to issues closed before cutoff.
- `--ephemeral` targets only closed wisps.

**JSON output**:
- Empty case: `{ deleted_count, message, filter?, ephemeral? }`.
- Otherwise uses delete batch JSON shape:
  `{ deleted, deleted_count, dependencies_removed, labels_removed,
     events_removed, references_updated, orphaned_issues }`.

#### 15.50.2 compact

**Modes**:
- `--prune`: age-based tombstone pruning.
- `--purge-tombstones`: dependency-aware pruning.
- `--analyze` / `--apply`: offline compaction workflow (no API key).
- `--auto`: AI compaction (requires API key; legacy).
- `--stats`: show compaction statistics.

**Port decision**:
- `br` v1 should exclude AI compaction and any remote API dependency.
- `prune`/`purge` may be kept as explicit maintenance commands only.

#### 15.50.3 migrate

**Default behavior**:
- Detects `.beads/*.db`, migrates or renames to configured DB name.
- Updates schema version metadata; can remove stale DBs with confirmation.

**Subcommands**:
- `hash-ids`, `issues`, `sync`, `tombstones` (legacy, often Gastown-related).

**Port decision**:
- `br` v1 keeps schema migration only (forward-compatible upgrades).
- Drop multi-repo sync-branch, hash-ids, and other Gastown-specific migrations.

---

### 15.51 Duplicates / Dedupe Workflows

**Command**: `bd duplicates`

**Grouping logic**:
- Groups open issues by content key: `{title, description, design, acceptance_criteria, status}`.
- Closed issues are excluded from grouping.

**Merge target selection**:
1. Highest dependent count (most referenced by other issues).
2. Highest text reference count (mentions in descriptions/notes).
3. Lexicographically smallest ID (tie-breaker).

**Output (JSON)**:
```
{
  "duplicate_groups": <int>,
  "groups": [
    {
      "title": "...",
      "issues": [
        {"id": "bd-1", "title": "...", "status": "open", "priority": 2,
         "references": 3, "dependents": 1, "is_merge_target": true}
      ],
      "suggested_target": "bd-1",
      "suggested_sources": ["bd-2"],
      "suggested_action": "bd close ... && bd dep add ... --type related",
      "note": "Duplicate: ..."
    }
  ],
  "merge_commands": ["..."]?,
  "merge_results": [ {"target":..., "sources":..., "closed":..., "linked":..., "errors":...} ]?
}
```

**Auto-merge**:
- `--auto-merge` closes sources and adds `related` deps to target.
- `--dry-run` prints merge commands without applying.

---

### 15.52 Orphans Command

**Definition**: issues referenced in git commits but still open/in_progress.

**Detection**:
- Runs `git log --oneline --all` and extracts IDs like `(bd-abc123)`.
- Filters to open/in_progress issues in DB.

**Output (JSON)**:
```
[
  {"issue_id": "bd-123", "title": "...", "status": "open",
   "latest_commit": "abc123", "latest_commit_message": "..."}
]
```

**Flags**:
- `--details`: include latest commit hash/message in human output.
- `--fix`: interactive confirmation, then runs `bd close <id>` per orphan.

---

### 15.53 ID Resolution, Prefix Semantics, and Rename

**Partial ID resolution** (`ResolvePartialID`):
- Accepts:
  - Full ID (`bd-abc123`)
  - Hash only (`abc123`)
  - Prefix without hyphen (`bdabc123`)
  - Hierarchical (`bd-abc123.1`)
- Resolution order:
  1. Exact ID match (SearchIssues w/ IDs filter).
  2. Normalize by prefix (configured `issue_prefix`) and retry exact match.
  3. Substring match against issue hash portion across all prefixes.
- Ambiguous matches produce an error listing candidate IDs.

**Prefix validation**:
- Explicit IDs must match `issue_prefix` or `allowed_prefixes`.
- `allowed_prefixes` is a comma-separated config value used for multi-prefix
  environments (Gastown).

**rename-prefix**:
- Validates new prefix (letters/numbers/hyphen, max length 8, trailing hyphen required).
- Detects multi-prefix corruption; requires `--repair` to consolidate.
- Updates IDs and all text references.
- Re-exports JSONL and resets metadata hashes.
- `--dry-run` prints sample mappings.

**Port note**:
- `br` v1 should keep partial ID resolution and strict prefix validation
  but may defer rename-prefix until after core commands stabilize.

---

### 15.54 Creation Routing and Cross-Repo Targets

**Auto routing**:
- `routing.mode = auto` uses user role detection:
  - Maintainer -> `routing.maintainer`
  - Contributor -> `routing.contributor`
- Falls back to `routing.default` or `.`.
- Legacy keys: `contributor.auto_route`, `contributor.planning_repo`.

**Explicit routing**:
- `--repo` overrides auto routing.
- Creates in target repo using direct mode and storage factory.

**Rig / prefix routing (Gastown)**:
- `--rig` or `--prefix` uses `routes.jsonl` at town root.
- Accepts rig name (`beads`), prefix with hyphen (`bd-`), or prefix without hyphen (`bd`).
- Resolves `.beads` directory for target rig and uses `PrefixOverride` for ID generation.

**Port decision**:
- `br` v1 keeps `--repo` as an explicit target option.
- `--rig` / `--prefix` routing (Gastown) is excluded from classic port.

---

### 15.55 Status / Type / Priority Customization

**Built-in statuses**: `open`, `in_progress`, `blocked`, `deferred`, `closed`, `tombstone`.

**Custom statuses**:
- Config key: `status.custom` (comma-separated), stored in DB config.
- Validated by `Status.IsValidWithCustom`.

**Built-in types (classic)**: `bug`, `feature`, `task`, `epic`, `chore`.

**Custom types**:
- Config key: `types.custom` (comma-separated), stored in DB config.
- Validated by `IssueType.IsValidWithCustom`.

**Priority**:
- Fixed range `0..4` (no custom ranges in classic).

**Port note**:
- `br` v1 keeps custom statuses and types.
- Gastown-only built-in types (agent/rig/gate/etc.) excluded.

---

### 15.56 Error Output and Exit Conventions

**Exit codes**:
- Success: `0`
- Fatal errors: `1`

**JSON error shape**:
- `FatalErrorRespectJSON` outputs `{ "error": "..." }` to stdout when `--json`.
- `outputJSONError` (auto-flush) outputs `{ "error": "...", "code"?: "..." }` to stderr.

**Stdout vs stderr**:
- Human-readable errors/warnings generally go to stderr.
- `--json` outputs are usually stdout, except:
  - `export --json` writes stats JSON to stderr to keep stdout for JSONL.
  - `import` ignores `--json` and writes human output to stderr.

**Port note**:
- `br` should keep this separation to avoid corrupting JSONL / JSON output.

---

### 15.57 No-DB Mode (`--no-db`)

**Purpose**: operate from JSONL without SQLite (memory-only).

**Initialization**:
- Locates `.beads/` (or `BEADS_DIR`).
- Reads `issues.jsonl` if present; otherwise starts empty.
- Detects prefix in order:
  1. `issue-prefix` in `config.yaml`
  2. Common prefix across all issues (must be consistent)
  3. Directory name fallback
- Mixed prefixes -> error (must set config explicitly).

**Runtime**:
- Uses in-memory storage for all operations.
- No daemon, no auto-import, no SQLite-specific features.

**Persistence**:
- At command exit, writes `issues.jsonl` atomically.
- Filters out ephemeral/wisp issues.

**Port note**:
- `br` should keep `--no-db` but limit features to safe read/write of JSONL.

---

## Appendix B: SQLite Connection String

```
file:path/to/beads.db?_pragma=foreign_keys(ON)&_pragma=busy_timeout(30000)&_pragma=journal_mode(WAL)
```

For rusqlite:

```rust
let conn = Connection::open_with_flags(
    path,
    OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
)?;

conn.pragma_update(None, "foreign_keys", "ON")?;
conn.pragma_update(None, "busy_timeout", 30000)?;
conn.pragma_update(None, "journal_mode", "WAL")?;
conn.pragma_update(None, "synchronous", "NORMAL")?;
conn.pragma_update(None, "cache_size", -65536)?; // 64MB
conn.pragma_update(None, "temp_store", "MEMORY")?;
```

---

## Appendix C: JSONL Example

Complete example of `.beads/issues.jsonl`:

```json
{"id":"bd-abc123","title":"Add user authentication","description":"Implement OAuth2 flow for user login","status":"in_progress","priority":1,"issue_type":"feature","assignee":"alice","created_at":"2024-01-10T10:00:00Z","updated_at":"2024-01-15T14:30:00Z","labels":["auth","security"],"dependencies":[{"issue_id":"bd-abc123","depends_on_id":"bd-xyz789","type":"blocks","created_at":"2024-01-10T10:00:00Z"}]}
{"id":"bd-def456","title":"Fix login button styling","description":"Button text is cut off on mobile","status":"open","priority":2,"issue_type":"bug","created_at":"2024-01-14T09:00:00Z","updated_at":"2024-01-14T09:00:00Z","labels":["ui","mobile"]}
{"id":"bd-xyz789","title":"Set up OAuth provider","status":"closed","priority":1,"issue_type":"task","created_at":"2024-01-08T08:00:00Z","updated_at":"2024-01-12T16:00:00Z","closed_at":"2024-01-12T16:00:00Z","close_reason":"Provider configured and tested"}
{"id":"bd-old001","title":"Deprecated feature","status":"tombstone","priority":3,"issue_type":"feature","created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-05T00:00:00Z","deleted_at":"2024-01-05T00:00:00Z","deleted_by":"admin","delete_reason":"Feature cancelled","original_type":"feature"}
```

---

*Document generated for beads_rust porting project.*
*This is the authoritative specification - consult this instead of Go source files.*
*Last updated: 2026-01-16*
