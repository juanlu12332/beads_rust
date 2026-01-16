//! `SQLite` storage implementation.

use crate::error::{BeadsError, Result};
use crate::format::{IssueDetails, IssueWithDependencyMetadata};
use crate::model::{Comment, Event, EventType, Issue, IssueType, Priority, Status};
use crate::storage::events::get_events;
use crate::storage::schema::apply_schema;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, Transaction};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

/// SQLite-based storage backend.
#[derive(Debug)]
pub struct SqliteStorage {
    conn: Connection,
}

/// Context for a mutation operation, tracking side effects.
pub struct MutationContext {
    pub op_name: String,
    pub actor: String,
    pub events: Vec<Event>,
    pub dirty_ids: HashSet<String>,
    pub invalidate_blocked_cache: bool,
}

impl MutationContext {
    #[must_use]
    pub fn new(op_name: &str, actor: &str) -> Self {
        Self {
            op_name: op_name.to_string(),
            actor: actor.to_string(),
            events: Vec::new(),
            dirty_ids: HashSet::new(),
            invalidate_blocked_cache: false,
        }
    }

    pub fn record_event(&mut self, event_type: EventType, issue_id: &str, details: Option<String>) {
        self.events.push(Event {
            id: 0, // Placeholder, DB assigns auto-inc ID
            issue_id: issue_id.to_string(),
            event_type,
            actor: self.actor.clone(),
            old_value: None,
            new_value: None,
            comment: details,
            created_at: Utc::now(),
        });
    }

    pub fn mark_dirty(&mut self, issue_id: &str) {
        self.dirty_ids.insert(issue_id.to_string());
    }

    pub const fn invalidate_cache(&mut self) {
        self.invalidate_blocked_cache = true;
    }
}

impl SqliteStorage {
    /// Open a new connection to the database at the given path.
    ///
    /// If the database does not exist, it will be created and the schema applied.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established or schema application fails.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Apply schema (idempotent)
        apply_schema(&conn)?;

        Ok(Self { conn })
    }

    /// Open an in-memory database for testing.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established.
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        apply_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Execute a mutation with the 4-step transaction protocol.
    ///
    /// 1. Begin IMMEDIATE transaction
    /// 2. Apply changes
    /// 3. Write events
    /// 4. Mark dirty
    /// 5. Invalidate cache (if needed)
    /// 6. Commit
    ///
    /// # Errors
    ///
    /// Returns an error if any step fails (e.g. database error, logic error).
    /// The transaction is rolled back on error.
    pub fn mutate<F, R>(&mut self, op: &str, actor: &str, f: F) -> Result<R>
    where
        F: FnOnce(&Transaction, &mut MutationContext) -> Result<R>,
    {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut ctx = MutationContext::new(op, actor);

        let result = f(&tx, &mut ctx)?;

        // Write events
        for event in ctx.events {
            tx.execute(
                "INSERT INTO events (issue_id, event_type, actor, old_value, new_value, comment, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    event.issue_id,
                    event.event_type.as_str(),
                    event.actor,
                    event.old_value,
                    event.new_value,
                    event.comment,
                    event.created_at.to_rfc3339()
                ],
            )?;
        }

        // Mark dirty
        for id in ctx.dirty_ids {
            tx.execute(
                "INSERT OR REPLACE INTO dirty_issues (issue_id, marked_at) VALUES (?, ?)",
                rusqlite::params![id, Utc::now().to_rfc3339()],
            )?;
        }

        // Invalidate cache
        if ctx.invalidate_blocked_cache {
            tx.execute("DELETE FROM blocked_issues_cache", [])?;
        }

        tx.commit()?;
        Ok(result)
    }

    /// Create a new issue.
    ///
    /// # Errors
    ///
    /// Returns an error if the issue cannot be inserted (e.g. ID collision).
    pub fn create_issue(&mut self, issue: &Issue, actor: &str) -> Result<()> {
        self.mutate("create_issue", actor, |tx, ctx| {
            tx.execute(
                "INSERT INTO issues (
                    id, title, description, status, priority, issue_type, 
                    assignee, owner, estimated_minutes, 
                    created_at, created_by, updated_at, 
                    due_at, defer_until, external_ref
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    issue.id,
                    issue.title,
                    issue.description,
                    issue.status.as_str(),
                    issue.priority.0,
                    issue.issue_type.as_str(),
                    issue.assignee,
                    issue.owner,
                    issue.estimated_minutes,
                    issue.created_at.to_rfc3339(),
                    issue.created_by,
                    issue.updated_at.to_rfc3339(),
                    issue.due_at.map(|t| t.to_rfc3339()),
                    issue.defer_until.map(|t| t.to_rfc3339()),
                    issue.external_ref,
                ],
            )?;

            ctx.record_event(
                EventType::Created,
                &issue.id,
                Some(format!("Created issue: {}", issue.title)),
            );

            ctx.mark_dirty(&issue.id);

            Ok(())
        })
    }

    /// Check if an issue ID already exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn id_exists(&self, id: &str) -> Result<bool> {
        let count: i64 =
            self.conn
                .query_row("SELECT count(*) FROM issues WHERE id = ?", [id], |row| {
                    row.get(0)
                })?;
        Ok(count > 0)
    }

    /// Count total issues in the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn count_issues(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM issues", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Get an issue by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_issue(&self, id: &str) -> Result<Option<Issue>> {
        let sql = r"
            SELECT id, content_hash, title, description, design, acceptance_criteria, notes,
                   status, priority, issue_type, assignee, owner, estimated_minutes,
                   created_at, created_by, updated_at, closed_at, close_reason, closed_by_session,
                   due_at, defer_until, external_ref, source_system,
                   deleted_at, deleted_by, delete_reason, original_type,
                   compaction_level, compacted_at, compacted_at_commit, original_size,
                   sender, ephemeral, pinned, is_template
            FROM issues WHERE id = ?
        ";

        let mut stmt = self.conn.prepare(sql)?;
        let result = stmt.query_row([id], |row| self.issue_from_row(row));

        match result {
            Ok(issue) => Ok(Some(issue)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List issues with optional filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn list_issues(&self, filters: &ListFilters) -> Result<Vec<Issue>> {
        let mut sql = String::from(
            r"SELECT id, content_hash, title, description, design, acceptance_criteria, notes,
                     status, priority, issue_type, assignee, owner, estimated_minutes,
                     created_at, created_by, updated_at, closed_at, close_reason, closed_by_session,
                     due_at, defer_until, external_ref, source_system,
                     deleted_at, deleted_by, delete_reason, original_type,
                     compaction_level, compacted_at, compacted_at_commit, original_size,
                     sender, ephemeral, pinned, is_template
              FROM issues WHERE 1=1",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Status filter
        if let Some(ref statuses) = filters.statuses {
            if !statuses.is_empty() {
                let placeholders: Vec<String> = statuses.iter().map(|_| "?".to_string()).collect();
                let _ = write!(sql, " AND status IN ({})", placeholders.join(","));
                for s in statuses {
                    params.push(Box::new(s.as_str().to_string()));
                }
            }
        }

        // Type filter
        if let Some(ref types) = filters.types {
            if !types.is_empty() {
                let placeholders: Vec<String> = types.iter().map(|_| "?".to_string()).collect();
                let _ = write!(sql, " AND issue_type IN ({})", placeholders.join(","));
                for t in types {
                    params.push(Box::new(t.as_str().to_string()));
                }
            }
        }

        // Priority filter
        if let Some(ref priorities) = filters.priorities {
            if !priorities.is_empty() {
                let placeholders: Vec<String> =
                    priorities.iter().map(|_| "?".to_string()).collect();
                let _ = write!(sql, " AND priority IN ({})", placeholders.join(","));
                for p in priorities {
                    params.push(Box::new(p.0));
                }
            }
        }

        // Assignee filter
        if let Some(ref assignee) = filters.assignee {
            sql.push_str(" AND assignee = ?");
            params.push(Box::new(assignee.clone()));
        }

        // Unassigned filter
        if filters.unassigned {
            sql.push_str(" AND assignee IS NULL");
        }

        // Exclude closed by default (unless include_closed is true)
        if !filters.include_closed {
            sql.push_str(" AND status NOT IN ('closed', 'tombstone')");
        }

        // Exclude templates by default
        if !filters.include_templates {
            sql.push_str(" AND (is_template = 0 OR is_template IS NULL)");
        }

        // Title contains filter
        if let Some(ref title_contains) = filters.title_contains {
            sql.push_str(" AND title LIKE ?");
            params.push(Box::new(format!("%{title_contains}%")));
        }

        // Ordering: priority ASC, created_at DESC by default
        sql.push_str(" ORDER BY priority ASC, created_at DESC");

        // Limit
        if let Some(limit) = filters.limit {
            if limit > 0 {
                let _ = write!(sql, " LIMIT {limit}");
            }
        }

        let mut stmt = self.conn.prepare(&sql)?;

        // Build params slice
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(AsRef::as_ref).collect();

        let issues = stmt
            .query_map(params_refs.as_slice(), |row| self.issue_from_row(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(issues)
    }


    /// Search issues by query with optional filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn search_issues(&self, query: &str, filters: &ListFilters) -> Result<Vec<Issue>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = String::from(
            r"SELECT id, content_hash, title, description, design, acceptance_criteria, notes,
                     status, priority, issue_type, assignee, owner, estimated_minutes,
                     created_at, created_by, updated_at, closed_at, close_reason, closed_by_session,
                     due_at, defer_until, external_ref, source_system,
                     deleted_at, deleted_by, delete_reason, original_type,
                     compaction_level, compacted_at, compacted_at_commit, original_size,
                     sender, ephemeral, pinned, is_template
              FROM issues WHERE 1=1",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        sql.push_str(" AND (title LIKE ? OR description LIKE ? OR id LIKE ?)");
        let pattern = format!("%{trimmed}%");
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));

        if let Some(ref statuses) = filters.statuses {
            if !statuses.is_empty() {
                let placeholders: Vec<String> = statuses.iter().map(|_| "?".to_string()).collect();
                let _ = write!(sql, " AND status IN ({})", placeholders.join(","));
                for s in statuses {
                    params.push(Box::new(s.as_str().to_string()));
                }
            }
        }

        if let Some(ref types) = filters.types {
            if !types.is_empty() {
                let placeholders: Vec<String> = types.iter().map(|_| "?".to_string()).collect();
                let _ = write!(sql, " AND issue_type IN ({})", placeholders.join(","));
                for t in types {
                    params.push(Box::new(t.as_str().to_string()));
                }
            }
        }

        if let Some(ref priorities) = filters.priorities {
            if !priorities.is_empty() {
                let placeholders: Vec<String> =
                    priorities.iter().map(|_| "?".to_string()).collect();
                let _ = write!(sql, " AND priority IN ({})", placeholders.join(","));
                for p in priorities {
                    params.push(Box::new(p.0));
                }
            }
        }

        if let Some(ref assignee) = filters.assignee {
            sql.push_str(" AND assignee = ?");
            params.push(Box::new(assignee.clone()));
        }

        if filters.unassigned {
            sql.push_str(" AND assignee IS NULL");
        }

        if !filters.include_closed {
            sql.push_str(" AND status NOT IN ('closed', 'tombstone')");
        }

        if !filters.include_templates {
            sql.push_str(" AND (is_template = 0 OR is_template IS NULL)");
        }

        if let Some(ref title_contains) = filters.title_contains {
            sql.push_str(" AND title LIKE ?");
            params.push(Box::new(format!("%{title_contains}%")));
        }

        sql.push_str(" ORDER BY priority ASC, created_at DESC");

        if let Some(limit) = filters.limit {
            if limit > 0 {
                let _ = write!(sql, " LIMIT {limit}");
            }
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(AsRef::as_ref).collect();
        let issues = stmt
            .query_map(params_refs.as_slice(), |row| self.issue_from_row(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(issues)
    }

    /// Count how many dependencies an issue has (issues this one depends on).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn count_dependencies(&self, issue_id: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM dependencies WHERE issue_id = ?",
            [issue_id],
            |row| row.get(0),
        )?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Count how many issues depend on this one (dependents).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn count_dependents(&self, issue_id: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM dependencies WHERE depends_on_id = ?",
            [issue_id],
            |row| row.get(0),
        )?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Get labels for an issue.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_labels(&self, issue_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT label FROM labels WHERE issue_id = ? ORDER BY label")?;
        let labels = stmt
            .query_map([issue_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(labels)
    }

    /// Get IDs of issues that depend on this one (dependents).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_dependents(&self, issue_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT issue_id FROM dependencies WHERE depends_on_id = ?")?;
        let ids = stmt
            .query_map([issue_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Get IDs of issues that this one depends on (dependencies).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_dependencies(&self, issue_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT depends_on_id FROM dependencies WHERE issue_id = ?")?;
        let ids = stmt
            .query_map([issue_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Delete an issue by creating a tombstone.
    ///
    /// This sets:
    /// - status = tombstone
    /// - `deleted_at` = now
    /// - `deleted_by` = actor
    /// - `delete_reason` = reason
    /// - `original_type` = previous `issue_type`
    ///
    /// Does NOT remove labels, comments, or events from DB.
    ///
    /// # Errors
    ///
    /// Returns an error if the issue doesn't exist or the update fails.
    pub fn delete_issue(&mut self, id: &str, actor: &str, reason: &str) -> Result<Issue> {
        // First get the existing issue to capture original_type
        let issue = self
            .get_issue(id)?
            .ok_or_else(|| BeadsError::IssueNotFound { id: id.to_string() })?;

        let original_type = issue.issue_type.as_str().to_string();

        self.mutate("delete_issue", actor, |tx, ctx| {
            tx.execute(
                "UPDATE issues SET
                    status = 'tombstone',
                    deleted_at = ?,
                    deleted_by = ?,
                    delete_reason = ?,
                    original_type = ?,
                    updated_at = ?
                 WHERE id = ?",
                rusqlite::params![
                    Utc::now().to_rfc3339(),
                    actor,
                    reason,
                    original_type,
                    Utc::now().to_rfc3339(),
                    id
                ],
            )?;

            ctx.record_event(
                EventType::Deleted,
                id,
                Some(format!("Deleted issue: {reason}")),
            );
            ctx.mark_dirty(id);
            ctx.invalidate_cache();

            Ok(())
        })?;

        // Return the updated issue
        self.get_issue(id)?
            .ok_or_else(|| BeadsError::IssueNotFound { id: id.to_string() })
    }

    /// Remove a dependency link.
    ///
    /// # Errors
    ///
    /// Returns an error if the database update fails.
    pub fn remove_dependency(
        &mut self,
        issue_id: &str,
        depends_on_id: &str,
        actor: &str,
    ) -> Result<bool> {
        self.mutate("remove_dependency", actor, |tx, ctx| {
            let rows = tx.execute(
                "DELETE FROM dependencies WHERE issue_id = ? AND depends_on_id = ?",
                rusqlite::params![issue_id, depends_on_id],
            )?;

            if rows > 0 {
                ctx.record_event(
                    EventType::DependencyRemoved,
                    issue_id,
                    Some(format!("Removed dependency on {depends_on_id}")),
                );
                ctx.mark_dirty(issue_id);
                ctx.invalidate_cache();
            }

            Ok(rows > 0)
        })
    }

    /// Add a dependency link.
    ///
    /// # Errors
    ///
    /// Returns an error if the database insert fails (e.g., duplicate).
    pub fn add_dependency(
        &mut self,
        issue_id: &str,
        depends_on_id: &str,
        dep_type: &str,
        actor: &str,
    ) -> Result<()> {
        self.mutate("add_dependency", actor, |tx, ctx| {
            tx.execute(
                "INSERT INTO dependencies (issue_id, depends_on_id, type, created_at, created_by)
                 VALUES (?, ?, ?, ?, ?)",
                rusqlite::params![
                    issue_id,
                    depends_on_id,
                    dep_type,
                    Utc::now().to_rfc3339(),
                    actor
                ],
            )?;

            ctx.record_event(
                EventType::DependencyAdded,
                issue_id,
                Some(format!("Added dependency on {depends_on_id}")),
            );
            ctx.mark_dirty(issue_id);
            ctx.invalidate_cache();

            Ok(())
        })
    }

    /// Add a label to an issue.
    ///
    /// Returns true if a new label was inserted.
    ///
    /// # Errors
    ///
    /// Returns an error if the database update fails.
    pub fn add_label(&mut self, issue_id: &str, label: &str, actor: &str) -> Result<bool> {
        self.mutate("add_label", actor, |tx, ctx| {
            let rows = tx.execute(
                "INSERT OR IGNORE INTO labels (issue_id, label) VALUES (?, ?)",
                rusqlite::params![issue_id, label],
            )?;

            if rows > 0 {
                ctx.record_event(
                    EventType::LabelAdded,
                    issue_id,
                    Some(format!("Added label {label}")),
                );
                ctx.mark_dirty(issue_id);
            }

            Ok(rows > 0)
        })
    }

    /// Remove all dependencies for an issue (both directions).
    ///
    /// Returns count of dependencies removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the database update fails.
    pub fn remove_all_dependencies(&mut self, issue_id: &str, actor: &str) -> Result<usize> {
        self.mutate("remove_all_dependencies", actor, |tx, ctx| {
            // Get affected issues before deleting (for dirty tracking)
            let mut stmt = tx.prepare(
                "SELECT DISTINCT issue_id FROM dependencies WHERE depends_on_id = ?
                 UNION
                 SELECT DISTINCT depends_on_id FROM dependencies WHERE issue_id = ?",
            )?;
            let affected: Vec<String> = stmt
                .query_map(rusqlite::params![issue_id, issue_id], |row| row.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            // Remove dependencies where this issue depends on others
            let outgoing = tx.execute("DELETE FROM dependencies WHERE issue_id = ?", [issue_id])?;

            // Remove dependencies where others depend on this issue
            let incoming = tx.execute(
                "DELETE FROM dependencies WHERE depends_on_id = ?",
                [issue_id],
            )?;

            let total = outgoing + incoming;

            if total > 0 {
                ctx.record_event(
                    EventType::DependencyRemoved,
                    issue_id,
                    Some(format!("Removed {total} dependency links")),
                );
                ctx.mark_dirty(issue_id);

                // Mark all affected issues as dirty
                for affected_id in affected {
                    ctx.mark_dirty(&affected_id);
                }

                ctx.invalidate_cache();
            }

            Ok(total)
        })
    }

    /// Helper to construct an Issue from a database row.
    #[allow(clippy::unused_self)] // May need self for loading relations in the future
    fn issue_from_row(&self, row: &rusqlite::Row) -> rusqlite::Result<Issue> {
        Ok(Issue {
            id: row.get(0)?,
            content_hash: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            design: row.get(4)?,
            acceptance_criteria: row.get(5)?,
            notes: row.get(6)?,
            status: parse_status(row.get::<_, Option<String>>(7)?.as_deref()),
            priority: Priority(row.get::<_, Option<i32>>(8)?.unwrap_or(2)),
            issue_type: parse_issue_type(row.get::<_, Option<String>>(9)?.as_deref()),
            assignee: row.get(10)?,
            owner: row.get(11)?,
            estimated_minutes: row.get(12)?,
            created_at: parse_datetime(&row.get::<_, String>(13)?),
            created_by: row.get(14)?,
            updated_at: parse_datetime(&row.get::<_, String>(15)?),
            closed_at: row
                .get::<_, Option<String>>(16)?
                .as_deref()
                .map(parse_datetime),
            close_reason: row.get(17)?,
            closed_by_session: row.get(18)?,
            due_at: row
                .get::<_, Option<String>>(19)?
                .as_deref()
                .map(parse_datetime),
            defer_until: row
                .get::<_, Option<String>>(20)?
                .as_deref()
                .map(parse_datetime),
            external_ref: row.get(21)?,
            source_system: row.get(22)?,
            deleted_at: row
                .get::<_, Option<String>>(23)?
                .as_deref()
                .map(parse_datetime),
            deleted_by: row.get(24)?,
            delete_reason: row.get(25)?,
            original_type: row.get(26)?,
            compaction_level: row.get(27)?,
            compacted_at: row
                .get::<_, Option<String>>(28)?
                .as_deref()
                .map(parse_datetime),
            compacted_at_commit: row.get(29)?,
            original_size: row.get(30)?,
            sender: row.get(31)?,
            ephemeral: row.get::<_, Option<i32>>(32)?.unwrap_or(0) != 0,
            pinned: row.get::<_, Option<i32>>(33)?.unwrap_or(0) != 0,
            is_template: row.get::<_, Option<i32>>(34)?.unwrap_or(0) != 0,
            labels: vec![],       // Loaded separately if needed
            dependencies: vec![], // Loaded separately if needed
            comments: vec![],     // Loaded separately if needed
        })
    }

    /// Get comments for an issue, ordered by `created_at` ASC (oldest first).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_comments(&self, issue_id: &str) -> Result<Vec<Comment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, issue_id, author, text, created_at
             FROM comments
             WHERE issue_id = ?
             ORDER BY created_at ASC",
        )?;

        let comments = stmt
            .query_map([issue_id], |row| {
                let created_at_str: String = row.get(4)?;
                let created_at = parse_datetime(&created_at_str);
                Ok(Comment {
                    id: row.get(0)?,
                    issue_id: row.get(1)?,
                    author: row.get(2)?,
                    body: row.get(3)?,
                    created_at,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(comments)
    }

    /// Get dependencies with metadata (issues this one depends on).
    ///
    /// Returns dependency info including the target issue's title, status, and priority.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_dependencies_with_metadata(
        &self,
        issue_id: &str,
    ) -> Result<Vec<IssueWithDependencyMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.depends_on_id, i.title, i.status, i.priority, d.type
             FROM dependencies d
             LEFT JOIN issues i ON d.depends_on_id = i.id
             WHERE d.issue_id = ?
             ORDER BY i.priority ASC, i.created_at DESC",
        )?;

        let deps = stmt
            .query_map([issue_id], |row| {
                Ok(IssueWithDependencyMetadata {
                    id: row.get(0)?,
                    title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    status: parse_status(row.get::<_, Option<String>>(2)?.as_deref()),
                    priority: Priority(row.get::<_, Option<i32>>(3)?.unwrap_or(2)),
                    dep_type: row
                        .get::<_, Option<String>>(4)?
                        .unwrap_or_else(|| "blocks".to_string()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(deps)
    }

    /// Get dependents with metadata (issues that depend on this one).
    ///
    /// Returns dependency info including the dependent issue's title, status, and priority.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_dependents_with_metadata(
        &self,
        issue_id: &str,
    ) -> Result<Vec<IssueWithDependencyMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.issue_id, i.title, i.status, i.priority, d.type
             FROM dependencies d
             LEFT JOIN issues i ON d.issue_id = i.id
             WHERE d.depends_on_id = ?
             ORDER BY i.priority ASC, i.created_at DESC",
        )?;

        let deps = stmt
            .query_map([issue_id], |row| {
                Ok(IssueWithDependencyMetadata {
                    id: row.get(0)?,
                    title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    status: parse_status(row.get::<_, Option<String>>(2)?.as_deref()),
                    priority: Priority(row.get::<_, Option<i32>>(3)?.unwrap_or(2)),
                    dep_type: row
                        .get::<_, Option<String>>(4)?
                        .unwrap_or_else(|| "blocks".to_string()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(deps)
    }

    /// Get parent issue ID (from `parent-child` dependency type).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_parent_id(&self, issue_id: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT depends_on_id FROM dependencies WHERE issue_id = ? AND type = 'parent-child'",
            [issue_id],
            |row| row.get(0),
        );

        match result {
            Ok(parent_id) => Ok(Some(parent_id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get full issue details for the show command.
    ///
    /// Fetches the issue and all related data: labels, dependencies, dependents,
    /// comments (optional), events (optional), and parent.
    ///
    /// # Arguments
    ///
    /// * `id` - Issue ID
    /// * `include_comments` - Whether to load comments
    /// * `include_events` - Whether to load events
    /// * `event_limit` - Maximum number of events to load (0 = unlimited)
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn get_issue_details(
        &self,
        id: &str,
        include_comments: bool,
        include_events: bool,
        event_limit: usize,
    ) -> Result<Option<IssueDetails>> {
        // Get the base issue
        let Some(issue) = self.get_issue(id)? else {
            return Ok(None);
        };

        // Load labels
        let labels = self.get_labels(id)?;

        // Load dependencies (issues this one depends on)
        let dependencies = self.get_dependencies_with_metadata(id)?;

        // Load dependents (issues that depend on this one)
        let dependents = self.get_dependents_with_metadata(id)?;

        // Load comments if requested
        let comments = if include_comments {
            self.get_comments(id)?
        } else {
            vec![]
        };

        // Load events if requested
        let events = if include_events {
            get_events(&self.conn, id, event_limit)?
        } else {
            vec![]
        };

        // Load parent
        let parent = self.get_parent_id(id)?;

        Ok(Some(IssueDetails {
            issue,
            labels,
            dependencies,
            dependents,
            comments,
            events,
            parent,
        }))
    }

    /// Get a reference to the underlying connection (for use with event queries).
    #[must_use]
    pub const fn connection(&self) -> &Connection {
        &self.conn
    }
}

/// Filter options for listing issues.
#[derive(Debug, Clone, Default)]
pub struct ListFilters {
    pub statuses: Option<Vec<Status>>,
    pub types: Option<Vec<IssueType>>,
    pub priorities: Option<Vec<Priority>>,
    pub assignee: Option<String>,
    pub unassigned: bool,
    pub include_closed: bool,
    pub include_templates: bool,
    pub title_contains: Option<String>,
    pub limit: Option<usize>,
}

fn parse_status(s: Option<&str>) -> Status {
    s.and_then(|s| s.parse().ok()).unwrap_or_default()
}

fn parse_issue_type(s: Option<&str>) -> IssueType {
    s.and_then(|s| s.parse().ok()).unwrap_or_default()
}

fn parse_datetime(s: &str) -> DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(s).map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Issue, IssueType, Priority, Status};

    #[test]
    fn test_open_memory() {
        let storage = SqliteStorage::open_memory();
        assert!(storage.is_ok());
    }

    #[test]
    fn test_create_issue() {
        let mut storage = SqliteStorage::open_memory().unwrap();
        let issue = Issue {
            id: "bd-1".to_string(),
            title: "Test Issue".to_string(),
            status: Status::Open,
            priority: Priority::MEDIUM,
            issue_type: IssueType::Task,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            // ... defaults ...
            content_hash: None,
            description: None,
            design: None,
            acceptance_criteria: None,
            notes: None,
            assignee: None,
            owner: None,
            estimated_minutes: None,
            created_by: None,
            closed_at: None,
            close_reason: None,
            closed_by_session: None,
            due_at: None,
            defer_until: None,
            external_ref: None,
            source_system: None,
            deleted_at: None,
            deleted_by: None,
            delete_reason: None,
            original_type: None,
            compaction_level: None,
            compacted_at: None,
            compacted_at_commit: None,
            original_size: None,
            sender: None,
            ephemeral: false,
            pinned: false,
            is_template: false,
            labels: vec![],
            dependencies: vec![],
            comments: vec![],
        };

        storage.create_issue(&issue, "tester").unwrap();

        // Verify it exists (raw query since get_issue not impl yet)
        let count: i64 = storage
            .conn
            .query_row(
                "SELECT count(*) FROM issues WHERE id = ?",
                ["bd-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Verify event
        let event_count: i64 = storage
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE issue_id = ?",
                ["bd-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 1);

        // Verify dirty
        let dirty_count: i64 = storage
            .conn
            .query_row(
                "SELECT count(*) FROM dirty_issues WHERE issue_id = ?",
                ["bd-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dirty_count, 1);
    }

    #[test]
    fn test_transaction_rollback_on_error() {
        let mut storage = SqliteStorage::open_memory().unwrap();

        // Try to create an issue that will fail validation (title too long)
        let result: crate::error::Result<()> = storage.mutate("test_fail", "tester", |tx, ctx| {
            // Insert successfully first
            tx.execute(
                "INSERT INTO issues (id, title, status, priority, issue_type, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    "bd-rollback",
                    "Valid title",
                    "open",
                    2,
                    "task",
                    Utc::now().to_rfc3339(),
                    Utc::now().to_rfc3339(),
                ],
            )?;
            ctx.mark_dirty("bd-rollback");

            // Now force an error
            Err(crate::error::BeadsError::IssueNotFound {
                id: "forced".into(),
            })
        });

        assert!(result.is_err());

        // Issue should NOT exist due to rollback
        let count: i64 = storage
            .conn
            .query_row(
                "SELECT count(*) FROM issues WHERE id = ?",
                ["bd-rollback"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "Issue should not exist after rollback");

        // Dirty marker should NOT exist due to rollback
        let dirty_count: i64 = storage
            .conn
            .query_row(
                "SELECT count(*) FROM dirty_issues WHERE issue_id = ?",
                ["bd-rollback"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            dirty_count, 0,
            "Dirty marker should not exist after rollback"
        );
    }

    #[test]
    fn test_dirty_issues_accumulate() {
        let mut storage = SqliteStorage::open_memory().unwrap();

        // Create first issue
        let issue1 = Issue {
            id: "bd-dirty1".to_string(),
            title: "First".to_string(),
            status: Status::Open,
            priority: Priority::MEDIUM,
            issue_type: IssueType::Task,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            content_hash: None,
            description: None,
            design: None,
            acceptance_criteria: None,
            notes: None,
            assignee: None,
            owner: None,
            estimated_minutes: None,
            created_by: None,
            closed_at: None,
            close_reason: None,
            closed_by_session: None,
            due_at: None,
            defer_until: None,
            external_ref: None,
            source_system: None,
            deleted_at: None,
            deleted_by: None,
            delete_reason: None,
            original_type: None,
            compaction_level: None,
            compacted_at: None,
            compacted_at_commit: None,
            original_size: None,
            sender: None,
            ephemeral: false,
            pinned: false,
            is_template: false,
            labels: vec![],
            dependencies: vec![],
            comments: vec![],
        };
        storage.create_issue(&issue1, "tester").unwrap();

        // Create second issue
        let issue2 = Issue {
            id: "bd-dirty2".to_string(),
            title: "Second".to_string(),
            ..issue1.clone()
        };
        storage.create_issue(&issue2, "tester").unwrap();

        // Both should be dirty
        let dirty_count: i64 = storage
            .conn
            .query_row("SELECT count(*) FROM dirty_issues", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dirty_count, 2, "Both issues should be marked dirty");

        // Clear dirty for one
        storage
            .conn
            .execute("DELETE FROM dirty_issues WHERE issue_id = ?", ["bd-dirty1"])
            .unwrap();

        // One should remain dirty
        let dirty_count: i64 = storage
            .conn
            .query_row("SELECT count(*) FROM dirty_issues", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dirty_count, 1, "One issue should remain dirty");
    }

    #[test]
    fn test_events_have_timestamps() {
        let mut storage = SqliteStorage::open_memory().unwrap();
        let issue = Issue {
            id: "bd-events".to_string(),
            title: "Event Test".to_string(),
            status: Status::Open,
            priority: Priority::MEDIUM,
            issue_type: IssueType::Task,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            content_hash: None,
            description: None,
            design: None,
            acceptance_criteria: None,
            notes: None,
            assignee: None,
            owner: None,
            estimated_minutes: None,
            created_by: None,
            closed_at: None,
            close_reason: None,
            closed_by_session: None,
            due_at: None,
            defer_until: None,
            external_ref: None,
            source_system: None,
            deleted_at: None,
            deleted_by: None,
            delete_reason: None,
            original_type: None,
            compaction_level: None,
            compacted_at: None,
            compacted_at_commit: None,
            original_size: None,
            sender: None,
            ephemeral: false,
            pinned: false,
            is_template: false,
            labels: vec![],
            dependencies: vec![],
            comments: vec![],
        };
        storage.create_issue(&issue, "tester").unwrap();

        // Verify event has timestamp
        let created_at: String = storage
            .conn
            .query_row(
                "SELECT created_at FROM events WHERE issue_id = ?",
                ["bd-events"],
                |row| row.get(0),
            )
            .unwrap();

        // Should be a valid RFC3339 timestamp
        assert!(
            chrono::DateTime::parse_from_rfc3339(&created_at).is_ok(),
            "Event timestamp should be valid RFC3339"
        );
    }

    #[test]
    fn test_blocked_cache_invalidation() {
        let mut storage = SqliteStorage::open_memory().unwrap();

        // Manually insert some cache data
        storage
            .conn
            .execute(
                "INSERT INTO blocked_issues_cache (issue_id, blocked_by_json) VALUES (?, ?)",
                ["bd-cached", r#"["bd-blocker"]"#],
            )
            .unwrap();

        // Verify cache has data
        let cache_count: i64 = storage
            .conn
            .query_row("SELECT count(*) FROM blocked_issues_cache", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(cache_count, 1);

        // Run a mutation that invalidates cache
        storage
            .mutate("invalidate_test", "tester", |_tx, ctx| {
                ctx.invalidate_cache();
                Ok(())
            })
            .unwrap();

        // Cache should be empty
        let cache_count: i64 = storage
            .conn
            .query_row("SELECT count(*) FROM blocked_issues_cache", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(cache_count, 0, "Cache should be cleared after invalidation");
    }
}
