#![forbid(unsafe_code)]

//! Transaction support for grouping multiple commands atomically.
//!
//! Transactions allow multiple operations to be grouped together and
//! treated as a single undoable unit. If any operation fails, all
//! previous operations in the transaction are rolled back.
//!
//! # Usage
//!
//! ```ignore
//! use ftui_runtime::undo::{HistoryManager, Transaction};
//!
//! let mut history = HistoryManager::default();
//!
//! // Begin a transaction
//! let mut txn = Transaction::begin("Format Document");
//!
//! // Add commands to the transaction
//! txn.push(normalize_whitespace_cmd)?;
//! txn.push(fix_indentation_cmd)?;
//! txn.push(sort_imports_cmd)?;
//!
//! // Commit the transaction to history
//! history.push(txn.commit());
//! ```
//!
//! # Nested Transactions
//!
//! Transactions can be nested using `TransactionScope`:
//!
//! ```ignore
//! let mut scope = TransactionScope::new(&mut history);
//!
//! // Outer transaction
//! scope.begin("Refactor");
//!
//! // Inner transaction
//! scope.begin("Rename Variable");
//! scope.execute(rename_cmd)?;
//! scope.commit()?;
//!
//! // More outer work
//! scope.execute(move_function_cmd)?;
//! scope.commit()?;
//! ```
//!
//! # Invariants
//!
//! 1. A committed transaction acts as a single command in history
//! 2. Rollback undoes all executed commands in reverse order
//! 3. Nested transactions must be committed/rolled back in order
//! 4. Empty transactions produce no history entry

use std::fmt;

use super::command::{CommandBatch, CommandError, CommandResult, UndoableCmd};
use super::history::HistoryManager;

/// Builder for creating a group of commands as a single transaction.
///
/// Commands added to a transaction are executed immediately. If any
/// command fails, all previously executed commands are rolled back.
///
/// When committed, the transaction becomes a single entry in history
/// that can be undone/redone atomically.
pub struct Transaction {
    /// The underlying command batch.
    batch: CommandBatch,
    /// Number of commands that have been successfully executed.
    executed_count: usize,
    /// Whether the transaction has been committed or rolled back.
    finalized: bool,
}

impl fmt::Debug for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("description", &self.batch.description())
            .field("command_count", &self.batch.len())
            .field("executed_count", &self.executed_count)
            .field("finalized", &self.finalized)
            .finish()
    }
}

impl Transaction {
    /// Begin a new transaction with the given description.
    #[must_use]
    pub fn begin(description: impl Into<String>) -> Self {
        Self {
            batch: CommandBatch::new(description),
            executed_count: 0,
            finalized: false,
        }
    }

    /// Execute a command and add it to the transaction.
    ///
    /// The command is executed immediately. If it fails, all previously
    /// executed commands are rolled back and the error is returned.
    ///
    /// # Errors
    ///
    /// Returns error if the command fails to execute.
    pub fn execute(&mut self, mut cmd: Box<dyn UndoableCmd>) -> CommandResult {
        if self.finalized {
            return Err(CommandError::InvalidState(
                "transaction already finalized".to_string(),
            ));
        }

        // Execute the command
        if let Err(e) = cmd.execute() {
            // Rollback on failure
            self.rollback();
            return Err(e);
        }

        // Add to batch (already executed)
        self.executed_count += 1;
        self.batch.push_executed(cmd);
        Ok(())
    }

    /// Add a pre-executed command to the transaction.
    ///
    /// Use this when the command has already been executed externally.
    /// The command will be undone on rollback and redone on redo.
    ///
    /// # Errors
    ///
    /// Returns error if the transaction is already finalized.
    pub fn add_executed(&mut self, cmd: Box<dyn UndoableCmd>) -> CommandResult {
        if self.finalized {
            return Err(CommandError::InvalidState(
                "transaction already finalized".to_string(),
            ));
        }

        self.executed_count += 1;
        self.batch.push_executed(cmd);
        Ok(())
    }

    /// Commit the transaction, returning it as a single undoable command.
    ///
    /// Returns `None` if the transaction is empty.
    #[must_use]
    pub fn commit(mut self) -> Option<Box<dyn UndoableCmd>> {
        self.finalized = true;

        if self.batch.is_empty() {
            None
        } else {
            // Take ownership of the batch, replacing with an empty one.
            // This works because Drop only rolls back if not finalized,
            // and we just set finalized = true.
            let batch = std::mem::replace(&mut self.batch, CommandBatch::new(""));
            Some(Box::new(batch))
        }
    }

    /// Roll back all executed commands in the transaction.
    ///
    /// This undoes all commands in reverse order. After rollback,
    /// the transaction is finalized and cannot be used further.
    pub fn rollback(&mut self) {
        if self.finalized {
            return;
        }

        // Rollback already happens in batch.undo(), but we need to
        // manually track that we're rolling back here
        // Since commands are in the batch but haven't been "undone" via
        // the batch's undo mechanism, we need to undo them directly.

        // The batch stores commands but doesn't track execution state
        // the same way we do. We need to undo the executed commands.
        // Since we can't easily access individual commands in the batch,
        // we rely on the batch's undo mechanism.

        // Mark as finalized before undo to prevent re-entry
        self.finalized = true;

        // If we have executed commands, undo them via the batch
        if self.executed_count > 0 {
            // The batch's undo will undo all commands
            let _ = self.batch.undo();
        }
    }

    /// Check if the transaction is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Get the number of commands in the transaction.
    #[must_use]
    pub fn len(&self) -> usize {
        self.batch.len()
    }

    /// Get the transaction description.
    #[must_use]
    pub fn description(&self) -> &str {
        self.batch.description()
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // If transaction wasn't finalized, auto-rollback
        if !self.finalized {
            self.rollback();
        }
    }
}

/// Scope-based transaction manager for nested transactions.
///
/// Provides a stack-based interface for managing nested transactions.
/// Each `begin()` pushes a new transaction, and `commit()` or `rollback()`
/// pops and finalizes it.
pub struct TransactionScope<'a> {
    /// Reference to the history manager.
    history: &'a mut HistoryManager,
    /// Stack of active transactions.
    stack: Vec<Transaction>,
}

impl<'a> TransactionScope<'a> {
    /// Create a new transaction scope.
    #[must_use]
    pub fn new(history: &'a mut HistoryManager) -> Self {
        Self {
            history,
            stack: Vec::new(),
        }
    }

    /// Begin a new nested transaction.
    pub fn begin(&mut self, description: impl Into<String>) {
        self.stack.push(Transaction::begin(description));
    }

    /// Execute a command in the current transaction.
    ///
    /// If no transaction is active, the command is executed and added
    /// directly to history.
    ///
    /// # Errors
    ///
    /// Returns error if the command fails.
    pub fn execute(&mut self, cmd: Box<dyn UndoableCmd>) -> CommandResult {
        if let Some(txn) = self.stack.last_mut() {
            txn.execute(cmd)
        } else {
            // No active transaction, execute directly
            let mut cmd = cmd;
            cmd.execute()?;
            self.history.push(cmd);
            Ok(())
        }
    }

    /// Commit the current transaction.
    ///
    /// If nested, the committed transaction is added to the parent.
    /// If at top level, it's added to history.
    ///
    /// # Errors
    ///
    /// Returns error if no transaction is active.
    pub fn commit(&mut self) -> CommandResult {
        let txn = self
            .stack
            .pop()
            .ok_or_else(|| CommandError::InvalidState("no active transaction".to_string()))?;

        if let Some(cmd) = txn.commit() {
            if let Some(parent) = self.stack.last_mut() {
                // Add to parent transaction as pre-executed
                parent.add_executed(cmd)?;
            } else {
                // Add to history
                self.history.push(cmd);
            }
        }

        Ok(())
    }

    /// Roll back the current transaction.
    ///
    /// # Errors
    ///
    /// Returns error if no transaction is active.
    pub fn rollback(&mut self) -> CommandResult {
        let mut txn = self
            .stack
            .pop()
            .ok_or_else(|| CommandError::InvalidState("no active transaction".to_string()))?;

        txn.rollback();
        Ok(())
    }

    /// Check if there are active transactions.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Get the current nesting depth.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Drop for TransactionScope<'_> {
    fn drop(&mut self) {
        // Auto-rollback any uncommitted transactions
        while let Some(mut txn) = self.stack.pop() {
            txn.rollback();
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::command::{TextInsertCmd, WidgetId};
    use crate::undo::history::HistoryConfig;
    use std::sync::Arc;
    use std::sync::Mutex;

    /// Helper to create a test command with a shared buffer.
    fn make_cmd(buffer: Arc<Mutex<String>>, text: &str) -> Box<dyn UndoableCmd> {
        let b1 = buffer.clone();
        let b2 = buffer.clone();
        let text = text.to_string();
        let text_clone = text.clone();

        let mut cmd = TextInsertCmd::new(WidgetId::new(1), 0, text)
            .with_apply(move |_, _, txt| {
                let mut buf = b1.lock().unwrap();
                buf.push_str(txt);
                Ok(())
            })
            .with_remove(move |_, _, _| {
                let mut buf = b2.lock().unwrap();
                buf.drain(..text_clone.len());
                Ok(())
            });

        cmd.execute().unwrap();
        Box::new(cmd)
    }

    #[test]
    fn test_empty_transaction() {
        let txn = Transaction::begin("Empty");
        assert!(txn.is_empty());
        assert_eq!(txn.len(), 0);
        assert!(txn.commit().is_none());
    }

    #[test]
    fn test_single_command_transaction() {
        let buffer = Arc::new(Mutex::new(String::new()));

        let mut txn = Transaction::begin("Single");
        txn.add_executed(make_cmd(buffer.clone(), "hello")).unwrap();

        assert_eq!(txn.len(), 1);

        let cmd = txn.commit();
        assert!(cmd.is_some());
    }

    #[test]
    fn test_transaction_rollback() {
        let buffer = Arc::new(Mutex::new(String::new()));

        let mut txn = Transaction::begin("Rollback Test");
        txn.add_executed(make_cmd(buffer.clone(), "hello")).unwrap();
        txn.add_executed(make_cmd(buffer.clone(), " world"))
            .unwrap();

        assert_eq!(*buffer.lock().unwrap(), "hello world");

        txn.rollback();

        // Buffer should be back to empty after rollback
        assert_eq!(*buffer.lock().unwrap(), "");
    }

    #[test]
    fn test_transaction_commit_to_history() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        let mut txn = Transaction::begin("Commit Test");
        txn.add_executed(make_cmd(buffer.clone(), "a")).unwrap();
        txn.add_executed(make_cmd(buffer.clone(), "b")).unwrap();

        if let Some(cmd) = txn.commit() {
            history.push(cmd);
        }

        assert_eq!(history.undo_depth(), 1);
        assert!(history.can_undo());
    }

    #[test]
    fn test_transaction_undo_redo() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        let mut txn = Transaction::begin("Undo/Redo Test");
        txn.add_executed(make_cmd(buffer.clone(), "hello")).unwrap();
        txn.add_executed(make_cmd(buffer.clone(), " world"))
            .unwrap();

        if let Some(cmd) = txn.commit() {
            history.push(cmd);
        }

        assert_eq!(*buffer.lock().unwrap(), "hello world");

        // Undo the entire transaction
        history.undo();
        assert_eq!(*buffer.lock().unwrap(), "");

        // Redo the entire transaction
        history.redo();
        assert_eq!(*buffer.lock().unwrap(), "hello world");
    }

    #[test]
    fn test_scope_basic() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        {
            let mut scope = TransactionScope::new(&mut history);
            scope.begin("Scope Test");

            scope.execute(make_cmd(buffer.clone(), "a")).unwrap();
            scope.execute(make_cmd(buffer.clone(), "b")).unwrap();

            scope.commit().unwrap();
        }

        assert_eq!(history.undo_depth(), 1);
    }

    #[test]
    fn test_scope_nested() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        {
            let mut scope = TransactionScope::new(&mut history);

            // Outer transaction
            scope.begin("Outer");
            scope.execute(make_cmd(buffer.clone(), "outer1")).unwrap();

            // Inner transaction
            scope.begin("Inner");
            scope.execute(make_cmd(buffer.clone(), "inner")).unwrap();
            scope.commit().unwrap();

            scope.execute(make_cmd(buffer.clone(), "outer2")).unwrap();
            scope.commit().unwrap();
        }

        // Both transactions committed as one (nested was added to parent)
        assert_eq!(history.undo_depth(), 1);
    }

    #[test]
    fn test_scope_rollback() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        {
            let mut scope = TransactionScope::new(&mut history);
            scope.begin("Rollback");

            scope.execute(make_cmd(buffer.clone(), "a")).unwrap();
            scope.execute(make_cmd(buffer.clone(), "b")).unwrap();

            scope.rollback().unwrap();
        }

        // Nothing should be in history
        assert_eq!(history.undo_depth(), 0);
    }

    #[test]
    fn test_scope_auto_rollback_on_drop() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        {
            let mut scope = TransactionScope::new(&mut history);
            scope.begin("Will be dropped");
            scope.execute(make_cmd(buffer.clone(), "test")).unwrap();
            // scope drops without commit
        }

        // Should have auto-rolled back
        assert_eq!(history.undo_depth(), 0);
    }

    #[test]
    fn test_scope_depth() {
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        let mut scope = TransactionScope::new(&mut history);
        assert_eq!(scope.depth(), 0);
        assert!(!scope.is_active());

        scope.begin("Level 1");
        assert_eq!(scope.depth(), 1);
        assert!(scope.is_active());

        scope.begin("Level 2");
        assert_eq!(scope.depth(), 2);

        scope.commit().unwrap();
        assert_eq!(scope.depth(), 1);

        scope.commit().unwrap();
        assert_eq!(scope.depth(), 0);
        assert!(!scope.is_active());
    }

    #[test]
    fn test_transaction_description() {
        let txn = Transaction::begin("My Transaction");
        assert_eq!(txn.description(), "My Transaction");
    }

    #[test]
    fn test_finalized_transaction_rejects_commands() {
        let buffer = Arc::new(Mutex::new(String::new()));

        let mut txn = Transaction::begin("Finalized");
        txn.rollback();

        let result = txn.add_executed(make_cmd(buffer, "test"));
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_execute_method() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let b1 = buffer.clone();
        let b2 = buffer.clone();

        let cmd = TextInsertCmd::new(WidgetId::new(1), 0, "exec")
            .with_apply(move |_, _, txt| {
                let mut buf = b1.lock().unwrap();
                buf.push_str(txt);
                Ok(())
            })
            .with_remove(move |_, _, _| {
                let mut buf = b2.lock().unwrap();
                buf.drain(..4);
                Ok(())
            });

        let mut txn = Transaction::begin("Execute Test");
        txn.execute(Box::new(cmd)).unwrap();
        assert_eq!(txn.len(), 1);
        assert_eq!(*buffer.lock().unwrap(), "exec");
    }

    #[test]
    fn test_transaction_finalized_rejects_execute() {
        let buffer = Arc::new(Mutex::new(String::new()));

        let mut txn = Transaction::begin("Finalized");
        txn.rollback();

        let result = txn.execute(make_cmd(buffer, "test"));
        assert!(result.is_err());
    }

    #[test]
    fn test_scope_execute_without_transaction() {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        {
            let mut scope = TransactionScope::new(&mut history);
            // Execute without begin() - should go directly to history
            scope.execute(make_cmd(buffer.clone(), "direct")).unwrap();
        }

        assert_eq!(history.undo_depth(), 1);
    }

    #[test]
    fn test_scope_commit_without_begin_errors() {
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        let mut scope = TransactionScope::new(&mut history);
        let result = scope.commit();
        assert!(result.is_err());
    }

    #[test]
    fn test_scope_rollback_without_begin_errors() {
        let mut history = HistoryManager::new(HistoryConfig::unlimited());

        let mut scope = TransactionScope::new(&mut history);
        let result = scope.rollback();
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_multi_command_rollback_order() {
        let buffer = Arc::new(Mutex::new(String::new()));

        let mut txn = Transaction::begin("Multi Rollback");
        txn.add_executed(make_cmd(buffer.clone(), "a")).unwrap();
        txn.add_executed(make_cmd(buffer.clone(), "b")).unwrap();
        txn.add_executed(make_cmd(buffer.clone(), "c")).unwrap();

        assert_eq!(*buffer.lock().unwrap(), "abc");
        txn.rollback();
        assert_eq!(*buffer.lock().unwrap(), "");
    }

    #[test]
    fn test_transaction_debug_impl() {
        let txn = Transaction::begin("Debug Test");
        let s = format!("{txn:?}");
        assert!(s.contains("Transaction"));
        assert!(s.contains("Debug Test"));
    }
}
