// ---------------------------------------------------------------------------
// Command dispatch against a scripted driver: the driver interface is already
// in place, so these tests cover test/exec without a database.
// ---------------------------------------------------------------------------

use dm_plugin_db::{ConnectionSpec, Database, DatabaseFactory, Outcome, QueryResult, Session};
use std::{cell::RefCell, rc::Rc};

/// Records what the commands asked for and answers with scripted outcomes.
#[derive(Default)]
pub(crate) struct DriverState {
    pub(crate) rows: Option<QueryResult>,
    pub(crate) affected: u64,
    pub(crate) run_error: Option<String>,
    pub(crate) connect_error: Option<String>,
    pub(crate) opened: RefCell<usize>,
    pub(crate) statements: RefCell<Vec<String>>,
    pub(crate) specs: RefCell<Vec<ConnectionSpec>>,
}

/// Handle given to the commands. Clones share one state, so the factory can hand
/// out a database that does not borrow the factory itself.
#[derive(Default, Clone)]
pub(crate) struct FakeDriver {
    state: Rc<DriverState>,
}

impl FakeDriver {
    pub(crate) fn new(state: DriverState) -> Self {
        Self {
            state: Rc::new(state),
        }
    }

    pub(crate) fn opened(&self) -> usize {
        *self.state.opened.borrow()
    }

    pub(crate) fn statements(&self) -> Vec<String> {
        self.state.statements.borrow().clone()
    }

    pub(crate) fn specs(&self) -> Vec<ConnectionSpec> {
        self.state.specs.borrow().clone()
    }
}

struct FakeDatabase {
    state: Rc<DriverState>,
}

struct FakeSession {
    state: Rc<DriverState>,
}

impl Database for FakeDatabase {
    fn connect(&self, spec: &ConnectionSpec) -> anyhow::Result<Box<dyn Session + '_>> {
        if let Some(error) = &self.state.connect_error {
            anyhow::bail!("{error}");
        }
        self.state.specs.borrow_mut().push(spec.clone());
        Ok(Box::new(FakeSession {
            state: Rc::clone(&self.state),
        }))
    }
}

impl Session for FakeSession {
    fn run(&mut self, sql: &str) -> anyhow::Result<Outcome> {
        self.state.statements.borrow_mut().push(sql.to_owned());
        if let Some(error) = &self.state.run_error {
            anyhow::bail!("{error}");
        }
        Ok(match &self.state.rows {
            Some(rows) => Outcome::Rows(rows.clone()),
            None => Outcome::Affected(self.state.affected),
        })
    }
}

impl DatabaseFactory for FakeDriver {
    fn open(&self) -> anyhow::Result<Box<dyn Database>> {
        *self.state.opened.borrow_mut() += 1;
        Ok(Box::new(FakeDatabase {
            state: Rc::clone(&self.state),
        }))
    }
}
