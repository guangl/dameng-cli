//! Unit tests for the database plugin: storage, configuration, prompts and
//! command dispatch. The driver behind "dm db test" and "dm db exec" is still a
//! placeholder, so these tests drive the commands with a scripted fake driver
//! and never need a database.

use std::{
    cell::RefCell, collections::VecDeque, ffi::OsString, fs, path::PathBuf, rc::Rc, time::Duration,
};

use dm_plugin_db::driver::PendingFactory;
use dm_plugin_db::{
    ConnectionSpec, DEFAULT_DRIVER, DEFAULT_PORT, DEFAULT_USERNAME, Database, DatabaseConnection,
    DatabaseFactory, DbConfig, Outcome, Prompter, QueryResult, Session, build_connection_string,
    config_path, connection_password, db_hint, decrypt, encrypt, find_connection, format_result,
    hex, load_config, load_connections, machine_key, open_database, read_sql, remove_connection,
    resolve_optional, resolve_password, resolve_port, resolve_required, resolve_with_default,
    run_with, run_with_prompter, unhex, upsert_connection, validate_name,
};
use dm_plugin_sdk::Context as PluginContext;
use tempfile::TempDir;

fn context(temp: &TempDir) -> PluginContext {
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    PluginContext {
        args: vec![OsString::from("db")],
        plugin_dir: temp.path().join("plugin"),
        home,
        config_dir: temp.path().join("config/db"),
        data_dir: temp.path().join("data/db"),
        cache_dir: temp.path().join("cache/db"),
        capabilities: vec!["config-dirs-v1".to_owned()],
    }
}

fn sample(name: &str) -> DatabaseConnection {
    DatabaseConnection {
        name: name.to_owned(),
        host: "10.0.0.8".to_owned(),
        port: 5236,
        username: "SYSDBA".to_owned(),
        schema: None,
        driver: "DM8 ODBC DRIVER".to_owned(),
        secret: None,
    }
}

fn with_args(context: &mut PluginContext, args: &[&str]) {
    context.args = args.iter().map(OsString::from).collect();
}

fn stored_connection(context: &PluginContext, name: &str, schema: Option<&str>) {
    let mut connection = sample(name);
    connection.schema = schema.map(str::to_owned);
    connection.secret = Some(encrypt(context, b"SYSDBA@123").unwrap());
    upsert_connection(context, &connection).unwrap();
}

#[test]
fn hex_and_secret_round_trip() {
    let bytes = b"secret-value";
    assert_eq!(unhex(&hex(bytes)).unwrap(), bytes);

    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let encrypted = encrypt(&context, b"SYSDBA@123").unwrap();
    assert_ne!(encrypted, "SYSDBA@123");
    assert_eq!(decrypt(&context, &encrypted).unwrap(), b"SYSDBA@123");
}

#[test]
fn hex_rejects_invalid_input() {
    assert!(unhex("0").is_err());
    assert!(unhex("zz").is_err());
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    assert!(decrypt(&context, "aa").is_err());
}

#[test]
fn machine_key_rejects_invalid_and_unreadable_key_files() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    fs::write(context.data_dir.join(".db-key"), b"too short").unwrap();
    assert!(machine_key(&context).is_err());

    fs::remove_file(context.data_dir.join(".db-key")).unwrap();
    fs::create_dir(context.data_dir.join(".db-key")).unwrap();
    assert!(machine_key(&context).is_err());
}

#[test]
fn decrypt_rejects_corrupt_ciphertext() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let secret = encrypt(&context, b"SYSDBA@123").unwrap();
    let mut bytes = unhex(&secret).unwrap();
    bytes.truncate(12);
    bytes.extend_from_slice(&[0_u8; 16]);
    assert!(decrypt(&context, &hex(&bytes)).is_err());
}

#[test]
fn validate_name_accepts_and_rejects() {
    assert!(validate_name("prod-01").is_ok());
    assert!(validate_name("").is_err());
    assert!(validate_name("../bad").is_err());
    assert!(validate_name("bad name").is_err());
    assert!(validate_name(&"a".repeat(65)).is_err());
}

#[test]
fn connection_store_round_trip() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let secret = encrypt(&context, b"SYSDBA@123").unwrap();
    let mut connection = sample("prod");
    connection.schema = Some("DMHR".to_owned());
    connection.secret = Some(secret);
    upsert_connection(&context, &connection).unwrap();

    let stored = load_connections(&context).unwrap();
    assert_eq!(stored, vec![connection.clone()]);
    assert_eq!(
        connection_password(&context, &stored[0]).unwrap(),
        "SYSDBA@123"
    );
    assert_eq!(find_connection(&context, "prod").unwrap().host, "10.0.0.8");

    // A second save replaces the row instead of adding one.
    let mut updated = connection.clone();
    updated.host = "10.0.0.9".to_owned();
    upsert_connection(&context, &updated).unwrap();
    assert_eq!(load_connections(&context).unwrap().len(), 1);
    assert_eq!(find_connection(&context, "prod").unwrap().host, "10.0.0.9");

    remove_connection(&context, "prod").unwrap();
    assert!(load_connections(&context).unwrap().is_empty());
}

#[test]
fn missing_and_corrupt_connections_are_reported() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    assert!(remove_connection(&context, "missing").is_err());
    let error = find_connection(&context, "missing").unwrap_err();
    assert!(
        format!("{error:#}").contains("is not configured"),
        "{error:#}"
    );

    let mut connection = sample("kept");
    upsert_connection(&context, &connection).unwrap();
    assert!(connection_password(&context, &connection).is_err());
    connection.secret = Some(encrypt(&context, b"pw").unwrap());
    assert_eq!(connection_password(&context, &connection).unwrap(), "pw");

    fs::create_dir_all(&context.data_dir).unwrap();
    let database = context.data_dir.join("connections.sqlite3");
    fs::remove_file(&database).unwrap();
    let sqlite = rusqlite::Connection::open(&database).unwrap();
    sqlite
        .execute_batch("CREATE TABLE connections (name TEXT PRIMARY KEY)")
        .unwrap();
    drop(sqlite);
    assert!(load_connections(&context).is_err());
    assert!(upsert_connection(&context, &sample("bad")).is_err());
}

#[cfg(unix)]
#[test]
fn open_database_reports_readonly_store_error() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    let database = context.data_dir.join("connections.sqlite3");
    drop(rusqlite::Connection::open(&database).unwrap());
    fs::set_permissions(&database, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(open_database(&context).is_err());
    fs::set_permissions(&database, fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn plugin_config_is_optional_and_validated() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);

    assert_eq!(
        config_path(&context),
        context.config_dir.join("config.toml")
    );
    assert_eq!(load_config(&context).unwrap(), DbConfig::default());

    fs::create_dir_all(&context.config_dir).unwrap();
    fs::write(
        config_path(&context),
        concat!(
            "[defaults]\nport = 5237\nusername = \"DMHR\"\ndriver = \"DM8 ODBC DRIVER\"\n",
            "schema = \"DMHR\"\n\n[connect]\ntimeout = 3\nprobe = \"SELECT 1 FROM DUAL\"\n",
        ),
    )
    .unwrap();
    let config = load_config(&context).unwrap();
    assert_eq!(config.defaults.port, Some(5237));
    assert_eq!(config.defaults.username.as_deref(), Some("DMHR"));
    assert_eq!(config.defaults.schema.as_deref(), Some("DMHR"));
    assert_eq!(config.connect.timeout, Some(3));
    assert_eq!(config.connect.probe.as_deref(), Some("SELECT 1 FROM DUAL"));

    for (text, expected) in [
        ("[defaults]\nport = 0\n", "defaults.port"),
        ("[connect]\ntimeout = 0\n", "greater than zero"),
        ("[defaults]\nusername = \"  \"\n", "must not be empty"),
        ("[defaults]\ndriver = \"\"\n", "must not be empty"),
        ("[defaults]\nschema = \" \"\n", "must not be empty"),
        ("[connect]\nprobe = \"\"\n", "must not be empty"),
        (
            "[connect]\nlibrary = \"/opt/dm/libdmdodbc.so\"\n",
            "Invalid database plugin configuration",
        ),
        (
            "[nope]\nport = 1\n",
            "Invalid database plugin configuration",
        ),
        ("port = 1\n", "Invalid database plugin configuration"),
    ] {
        fs::write(config_path(&context), text).unwrap();
        let error = load_config(&context).unwrap_err();
        assert!(
            format!("{error:#}").contains(expected),
            "{text} -> {error:#}"
        );
    }
}

#[test]
fn shipped_example_config_is_accepted() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.config_dir).unwrap();
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.toml");
    fs::write(config_path(&context), fs::read_to_string(&example).unwrap()).unwrap();

    let config = load_config(&context).unwrap();
    assert_eq!(config.defaults.port, Some(5236));
    assert_eq!(config.defaults.username.as_deref(), Some("SYSDBA"));
    assert_eq!(config.defaults.driver.as_deref(), Some("DM8 ODBC DRIVER"));
    assert_eq!(config.connect.timeout, Some(10));
    assert_eq!(config.connect.probe.as_deref(), Some("SELECT 1"));
}

#[test]
fn connection_string_covers_the_supported_fields() {
    let mut connection = sample("prod");
    assert_eq!(
        build_connection_string(&connection, "SYSDBA@123").unwrap(),
        "Driver={DM8 ODBC DRIVER};Server=10.0.0.8;Port=5236;UID=SYSDBA;PWD=SYSDBA@123;"
    );

    connection.schema = Some("DMHR".to_owned());
    assert_eq!(
        build_connection_string(&connection, "pw").unwrap(),
        "Driver={DM8 ODBC DRIVER};Server=10.0.0.8;Port=5236;UID=SYSDBA;PWD=pw;Schema=DMHR;"
    );

    // Separators and braces in the password are quoted.
    assert!(
        build_connection_string(&connection, "p;w}d")
            .unwrap()
            .contains("PWD={p;w}}d};")
    );
}

#[test]
fn connection_string_rejects_invalid_fields() {
    let mut connection = sample("prod");
    connection.host = "bad;host".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.username = "bad{user}".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.schema = Some("bad;schema".to_owned());
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.driver = "  ".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.host = "  ".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.username = String::new();
    assert!(build_connection_string(&connection, "pw").is_err());
}

/// Scripted answers for the interactive branches, which a test process cannot
/// drive through a real terminal.
struct Script {
    lines: RefCell<VecDeque<String>>,
    secrets: RefCell<VecDeque<String>>,
}

impl Script {
    fn new(lines: &[&str], secrets: &[&str]) -> Self {
        Self {
            lines: RefCell::new(lines.iter().map(|line| (*line).to_owned()).collect()),
            secrets: RefCell::new(secrets.iter().map(|secret| (*secret).to_owned()).collect()),
        }
    }

    /// Next scripted answer; an exhausted script fails like a closed terminal.
    fn take(queue: &RefCell<VecDeque<String>>) -> anyhow::Result<String> {
        queue
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("terminal closed"))
    }
}

impl Prompter for Script {
    fn line(&self, prompt: &str) -> anyhow::Result<String> {
        assert!(!prompt.is_empty(), "prompts must be visible text");
        Self::take(&self.lines)
    }

    fn secret(&self, prompt: &str) -> anyhow::Result<String> {
        assert!(!prompt.is_empty(), "prompts must be visible text");
        Self::take(&self.secrets)
    }
}

#[test]
fn prompts_supply_required_and_defaulted_values() {
    let script = Script::new(&["prod", "", "5237", "DMHR", "", ""], &["SYSDBA@123"]);
    let prompter = Some(&script as &dyn Prompter);

    assert_eq!(
        resolve_required(None, "Name: ", "name is required", prompter).unwrap(),
        "prod"
    );
    assert_eq!(resolve_port(None, None, prompter).unwrap(), DEFAULT_PORT);
    assert_eq!(resolve_port(None, None, prompter).unwrap(), 5237);
    assert_eq!(resolve_port(Some(6000), None, prompter).unwrap(), 6000);
    assert_eq!(
        resolve_with_default(None, None, DEFAULT_USERNAME, "Username: ", prompter).unwrap(),
        "DMHR"
    );
    assert_eq!(
        resolve_with_default(None, None, DEFAULT_DRIVER, "Driver: ", prompter).unwrap(),
        DEFAULT_DRIVER
    );
    assert_eq!(resolve_optional(None, "Schema: ", prompter).unwrap(), None);
    assert_eq!(resolve_password(None, prompter).unwrap(), "SYSDBA@123");
}

#[test]
fn prompts_fall_back_without_a_terminal() {
    assert_eq!(
        resolve_required(Some("prod".to_owned()), "Name: ", "missing", None).unwrap(),
        "prod"
    );
    assert!(resolve_required(None, "Name: ", "name is required", None).is_err());
    assert_eq!(resolve_port(None, None, None).unwrap(), DEFAULT_PORT);
    assert_eq!(resolve_port(None, Some(5300), None).unwrap(), 5300);
    assert_eq!(
        resolve_with_default(None, Some("DMHR".to_owned()), DEFAULT_USERNAME, "U: ", None).unwrap(),
        "DMHR"
    );
    assert_eq!(
        resolve_with_default(None, None, DEFAULT_USERNAME, "U: ", None).unwrap(),
        DEFAULT_USERNAME
    );
    assert!(
        resolve_with_default(Some("  ".to_owned()), None, DEFAULT_USERNAME, "U: ", None).is_err()
    );
    assert_eq!(resolve_optional(None, "Schema: ", None).unwrap(), None);
    assert_eq!(
        resolve_optional(Some(" DMHR ".to_owned()), "Schema: ", None)
            .unwrap()
            .as_deref(),
        Some("DMHR")
    );
    assert_eq!(
        resolve_optional(Some("  ".to_owned()), "Schema: ", None).unwrap(),
        None
    );
    assert_eq!(resolve_password(Some("pw".to_owned()), None).unwrap(), "pw");
    assert!(resolve_password(Some(String::new()), None).is_err());
    let error = resolve_password(None, None).unwrap_err();
    assert!(error.to_string().contains("--password"), "{error}");
}

#[test]
fn prompts_report_invalid_answers() {
    let script = Script::new(&["not-a-port"], &[""]);
    let prompter = Some(&script as &dyn Prompter);
    let error = resolve_port(None, None, prompter).unwrap_err();
    assert!(error.to_string().contains("must be a number"), "{error:#}");

    let error = resolve_password(None, prompter).unwrap_err();
    assert!(error.to_string().contains("must not be empty"), "{error:#}");
}

#[test]
fn sql_is_read_from_argument_file_or_rejected() {
    assert_eq!(
        read_sql(Some(" SELECT 1 ".to_owned()), None).unwrap(),
        "SELECT 1"
    );
    assert!(read_sql(Some("   ".to_owned()), None).is_err());

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("query.sql");
    fs::write(&file, "SELECT 2 FROM DUAL;\n").unwrap();
    assert_eq!(read_sql(None, Some(file)).unwrap(), "SELECT 2 FROM DUAL;");

    let missing = temp.path().join("missing.sql");
    assert!(read_sql(None, Some(missing)).is_err());
    // An explicit statement wins over a file that does not exist.
    assert_eq!(
        read_sql(
            Some("SELECT 3".to_owned()),
            Some(PathBuf::from("/missing.sql"))
        )
        .unwrap(),
        "SELECT 3"
    );
}

#[test]
fn results_are_rendered_as_tab_separated_rows() {
    let result = QueryResult {
        columns: vec!["ID".to_owned(), "NAME".to_owned()],
        rows: vec![
            vec![Some("1".to_owned()), Some("DM".to_owned())],
            vec![Some("2".to_owned()), None],
        ],
    };
    assert_eq!(format_result(&result), "ID\tNAME\n1\tDM\n2\t\n");
    assert_eq!(
        format_result(&QueryResult::default()),
        "\n",
        "a result without columns still prints one empty header line"
    );
}

// ---------------------------------------------------------------------------
// Command dispatch against a scripted driver: the driver interface is already
// in place, so these tests cover test/exec without a database.
// ---------------------------------------------------------------------------

/// Records what the commands asked for and answers with scripted outcomes.
#[derive(Default)]
struct DriverState {
    rows: Option<QueryResult>,
    affected: u64,
    run_error: Option<String>,
    connect_error: Option<String>,
    opened: RefCell<usize>,
    statements: RefCell<Vec<String>>,
    specs: RefCell<Vec<ConnectionSpec>>,
}

/// Handle given to the commands. Clones share one state, so the factory can hand
/// out a database that does not borrow the factory itself.
#[derive(Default, Clone)]
struct FakeDriver {
    state: Rc<DriverState>,
}

impl FakeDriver {
    fn new(state: DriverState) -> Self {
        Self {
            state: Rc::new(state),
        }
    }

    fn opened(&self) -> usize {
        *self.state.opened.borrow()
    }

    fn statements(&self) -> Vec<String> {
        self.state.statements.borrow().clone()
    }

    fn specs(&self) -> Vec<ConnectionSpec> {
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

#[test]
fn add_list_and_remove_run_without_any_driver() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    let driver = FakeDriver::default();

    with_args(
        &mut context,
        &[
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--port",
            "5237",
            "--username",
            "SYSDBA",
            "--password",
            "SYSDBA@123",
            "--schema",
            "DMHR",
            "--driver",
            "DM8 ODBC DRIVER",
        ],
    );
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    let stored = find_connection(&context, "prod").unwrap();
    assert_eq!(stored.port, 5237);
    assert_eq!(stored.schema.as_deref(), Some("DMHR"));
    assert_eq!(
        connection_password(&context, &stored).unwrap(),
        "SYSDBA@123"
    );

    with_args(&mut context, &["list"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);

    with_args(&mut context, &["remove", "prod"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert!(load_connections(&context).unwrap().is_empty());
    assert_eq!(driver.opened(), 0, "these commands never open a driver");
}

#[test]
fn add_uses_the_plugin_configuration_as_defaults() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    fs::create_dir_all(&context.config_dir).unwrap();
    fs::write(
        config_path(&context),
        concat!(
            "[defaults]\nport = 5300\nusername = \"DMHR\"\n",
            "driver = \"DM8 ODBC DRIVER\"\nschema = \"DMHR\"\n",
        ),
    )
    .unwrap();

    with_args(
        &mut context,
        &["add", "prod", "--host", "10.0.0.8", "--password", "pw"],
    );
    assert_eq!(run_with(&context, &FakeDriver::default()).unwrap(), 0);
    let stored = find_connection(&context, "prod").unwrap();
    assert_eq!(stored.port, 5300);
    assert_eq!(stored.username, "DMHR");
    assert_eq!(stored.schema.as_deref(), Some("DMHR"));

    // The documented defaults apply when neither flag nor file sets a value.
    fs::write(config_path(&context), "[defaults]\nport = 5300\n").unwrap();
    with_args(
        &mut context,
        &["add", "other", "--host", "10.0.0.9", "--password", "pw"],
    );
    assert_eq!(run_with(&context, &FakeDriver::default()).unwrap(), 0);
    let stored = find_connection(&context, "other").unwrap();
    assert_eq!(stored.username, DEFAULT_USERNAME);
    assert_eq!(stored.driver, DEFAULT_DRIVER);
    assert_eq!(stored.schema, None);
}

#[test]
fn add_rejects_invalid_input() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);

    with_args(
        &mut context,
        &["add", "bad name", "--host", "h", "--password", "pw"],
    );
    assert!(run_with(&context, &FakeDriver::default()).is_err());

    with_args(
        &mut context,
        &["add", "prod", "--host", "  ", "--password", "pw"],
    );
    assert!(run_with(&context, &FakeDriver::default()).is_err());

    with_args(&mut context, &["add", "prod", "--host", "h"]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(error.to_string().contains("--password"), "{error}");
}

#[test]
fn interactive_add_fills_in_every_missing_value() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    // Name, host, port (Enter keeps 5236), username, driver and schema.
    let script = Script::new(&["prod", "10.0.0.8", "", "DMHR", "", ""], &["SYSDBA@123"]);

    with_args(&mut context, &["add"]);
    assert_eq!(
        run_with_prompter(&context, &FakeDriver::default(), Some(&script)).unwrap(),
        0
    );

    let stored = find_connection(&context, "prod").unwrap();
    assert_eq!(stored.host, "10.0.0.8");
    assert_eq!(stored.port, DEFAULT_PORT);
    assert_eq!(stored.username, "DMHR");
    assert_eq!(stored.driver, DEFAULT_DRIVER);
    assert_eq!(stored.schema, None);
    assert_eq!(
        connection_password(&context, &stored).unwrap(),
        "SYSDBA@123"
    );
}

#[test]
fn interactive_add_reports_a_closed_terminal() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);

    // Every prefix of the answers makes a later prompt fail, so each interactive
    // step reports the closed terminal and nothing is saved.
    for answers in [
        &[][..],
        &["prod"][..],
        &["prod", "10.0.0.8"][..],
        &["prod", "10.0.0.8", ""][..],
        &["prod", "10.0.0.8", "", ""][..],
        &["prod", "10.0.0.8", "", "", ""][..],
    ] {
        let script = Script::new(answers, &[]);
        with_args(&mut context, &["add"]);
        let error = run_with_prompter(&context, &FakeDriver::default(), Some(&script)).unwrap_err();
        assert!(
            format!("{error:#}").contains("terminal closed"),
            "{error:#}"
        );
        assert!(load_connections(&context).unwrap().is_empty());
    }
}

#[test]
fn unreadable_plugin_configuration_is_reported() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    // A directory in place of the file makes reading it fail.
    fs::create_dir_all(config_path(&context)).unwrap();
    let error = load_config(&context).unwrap_err();
    assert!(format!("{error:#}").contains("config.toml"), "{error:#}");
}

#[test]
fn test_command_connects_and_runs_the_probe() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", Some("DMHR"));
    fs::create_dir_all(&context.config_dir).unwrap();
    fs::write(
        config_path(&context),
        "[connect]\ntimeout = 7\nprobe = \"SELECT 1 FROM DUAL\"\n",
    )
    .unwrap();

    let driver = FakeDriver::default();
    with_args(&mut context, &["test", "prod"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.opened(), 1);
    assert_eq!(driver.statements(), vec!["SELECT 1 FROM DUAL".to_owned()]);
    let specs = driver.specs();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].login_timeout, Duration::from_secs(7));
    assert!(specs[0].connection_string.contains("PWD=SYSDBA@123;"));
    assert!(specs[0].connection_string.contains("Schema=DMHR;"));
}

#[test]
fn test_command_uses_the_default_probe_without_configuration() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);
    let driver = FakeDriver::default();
    with_args(&mut context, &["test", "prod"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.statements(), vec!["SELECT 1".to_owned()]);
    let specs = driver.specs();
    assert_eq!(specs[0].login_timeout, Duration::from_secs(10));
    assert!(!specs[0].connection_string.contains("Schema="));
}

#[test]
fn test_command_reports_missing_connections_and_driver_failures() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);

    with_args(&mut context, &["test", "missing"]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(format!("{error:#}").contains("not configured"), "{error:#}");
    assert!(db_hint(&error).contains("dm db add"));

    // A stored connection without a password cannot connect.
    upsert_connection(&context, &sample("nopass")).unwrap();
    with_args(&mut context, &["test", "nopass"]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(format!("{error:#}").contains("password"), "{error:#}");

    // Driver-level failures surface unchanged.
    stored_connection(&context, "prod", None);
    let driver = FakeDriver::new(DriverState {
        connect_error: Some("Connect through the driver failed".to_owned()),
        ..DriverState::default()
    });
    with_args(&mut context, &["test", "prod"]);
    let error = run_with(&context, &driver).unwrap_err();
    assert!(
        format!("{error:#}").contains("Connect through the driver failed"),
        "{error:#}"
    );
}

#[test]
fn exec_prints_rows_and_affected_counts() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);

    let driver = FakeDriver::new(DriverState {
        rows: Some(QueryResult {
            columns: vec!["ID".to_owned()],
            rows: vec![vec![Some("1".to_owned())]],
        }),
        ..DriverState::default()
    });
    with_args(&mut context, &["exec", "prod", "SELECT ID FROM T"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.statements(), vec!["SELECT ID FROM T".to_owned()]);

    let driver = FakeDriver::new(DriverState {
        affected: 2,
        ..DriverState::default()
    });
    with_args(&mut context, &["exec", "prod", "UPDATE T SET A = 1"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
}

#[test]
fn exec_reads_sql_from_a_file_and_reports_failures() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);
    let file = temp.path().join("script.sql");
    fs::write(&file, "SELECT 1;\n").unwrap();

    let driver = FakeDriver::new(DriverState {
        affected: 1,
        ..DriverState::default()
    });
    with_args(
        &mut context,
        &["exec", "prod", "--file", file.to_str().unwrap()],
    );
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.statements(), vec!["SELECT 1;".to_owned()]);

    // Empty SQL fails before the store or the driver is touched.
    with_args(&mut context, &["exec", "prod", "   "]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(
        format!("{error:#}").contains("SQL must not be empty"),
        "{error:#}"
    );

    // Unknown connections are reported even when the SQL is fine.
    with_args(&mut context, &["exec", "missing", "SELECT 1"]);
    assert!(run_with(&context, &FakeDriver::default()).is_err());

    // Statement failures surface as errors.
    let driver = FakeDriver::new(DriverState {
        run_error: Some("Execute SQL failed".to_owned()),
        ..DriverState::default()
    });
    with_args(&mut context, &["exec", "prod", "SELECT 1"]);
    assert!(run_with(&context, &driver).is_err());
}

#[test]
fn the_pending_driver_reports_that_it_is_not_implemented() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);

    // test and exec reach the driver and report the deferred implementation.
    for args in [vec!["test", "prod"], vec!["exec", "prod", "SELECT 1"]] {
        with_args(&mut context, &args);
        let error = run_with(&context, &PendingFactory).unwrap_err();
        assert!(
            format!("{error:#}").contains("not implemented"),
            "{error:#}"
        );
        assert!(db_hint(&error).contains("尚未接入"), "{}", db_hint(&error));
    }

    // The commands that never open a driver keep working.
    with_args(&mut context, &["list"]);
    assert_eq!(run_with(&context, &PendingFactory).unwrap(), 0);
}

#[test]
fn hints_cover_every_reported_failure_class() {
    let hint = |message: &str| db_hint(&anyhow::anyhow!(message.to_owned()));
    assert!(hint("Database connection 'prod' is not configured").contains("dm db add"));
    assert!(hint("The database driver is not implemented yet").contains("尚未接入"));
    assert!(hint("The driver name must not be empty").contains("必填项"));
    assert!(hint("no such table: connections").contains("connections.sqlite3"));
    assert!(
        hint("table connections has no column named host: SQL logic error")
            .contains("connections.sqlite3")
    );
    assert!(hint("something else entirely").contains("dm db --help"));
}

#[test]
fn the_plugin_entry_point_reports_errors() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    with_args(&mut context, &["remove", "missing"]);
    let error = dm_plugin_db::run_cli(&context).unwrap_err();
    assert!(format!("{error:#}").contains("not configured"), "{error:#}");
}

#[test]
fn default_constants_match_the_documented_values() {
    assert_eq!(DEFAULT_PORT, 5236);
    assert_eq!(DEFAULT_USERNAME, "SYSDBA");
    assert_eq!(DEFAULT_DRIVER, "DM8 ODBC DRIVER");
}
