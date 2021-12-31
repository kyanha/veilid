use crate::intf::table_db::*;
use crate::intf::*;
use crate::*;
use keyvaluedb_sqlite::*;
use std::path::PathBuf;

struct TableStoreInner {
    config: VeilidConfig,
    opened: BTreeMap<String, Weak<Mutex<TableDBInner>>>,
}

#[derive(Clone)]
pub struct TableStore {
    inner: Arc<Mutex<TableStoreInner>>,
}

impl TableStore {
    fn new_inner(config: VeilidConfig) -> TableStoreInner {
        TableStoreInner {
            config,
            opened: BTreeMap::new(),
        }
    }
    pub fn new(config: VeilidConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(config))),
        }
    }

    pub async fn init(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn terminate(&self) {
        assert!(
            self.inner.lock().opened.is_empty(),
            "all open databases should have been closed"
        );
    }

    pub fn on_table_db_drop(&self, table: String) {
        let mut inner = self.inner.lock();
        if inner.opened.remove(&table).is_none() {
            unreachable!("should have removed an item");
        }
    }

    fn get_dbpath(inner: &TableStoreInner, table: &str) -> Result<PathBuf, String> {
        if !table
            .chars()
            .all(|c| char::is_alphanumeric(c) || c == '_' || c == '-')
        {
            return Err(format!("table name '{}' is invalid", table));
        }
        let c = inner.config.get();
        let tablestoredir = c.tablestore.directory.clone();
        std::fs::create_dir_all(&tablestoredir)
            .map_err(|e| format!("failed to create tablestore path: {}", e))?;

        let dbpath: PathBuf = [tablestoredir, String::from(table)].iter().collect();
        Ok(dbpath)
    }

    fn get_table_name(inner: &TableStoreInner, table: &str) -> Result<String, String> {
        if !table
            .chars()
            .all(|c| char::is_alphanumeric(c) || c == '_' || c == '-')
        {
            return Err(format!("table name '{}' is invalid", table));
        }
        let c = inner.config.get();
        let namespace = c.namespace.clone();
        Ok(if namespace.is_empty() {
            table.to_string()
        } else {
            format!("_ns_{}_{}", namespace, table)
        })
    }

    pub async fn open(&self, name: &str, column_count: u32) -> Result<TableDB, String> {
        let mut inner = self.inner.lock();
        let table_name = Self::get_table_name(&*inner, name)?;

        if let Some(table_db_weak_inner) = inner.opened.get(&table_name) {
            match TableDB::try_new_from_weak_inner(table_db_weak_inner.clone()) {
                Some(tdb) => {
                    return Ok(tdb);
                }
                None => {
                    inner.opened.remove(&table_name);
                }
            };
        }

        let dbpath = Self::get_dbpath(&inner, &table_name)?;
        let cfg = DatabaseConfig::with_columns(column_count);
        let db =
            Database::open(&dbpath, cfg).map_err(|e| format!("failed to open tabledb: {}", e))?;
        info!(
            "opened table store '{}' at path '{:?}' with {} columns",
            name, dbpath, column_count
        );
        let table_db = TableDB::new(table_name.clone(), self.clone(), db);

        inner.opened.insert(table_name, table_db.weak_inner());

        Ok(table_db)
    }

    pub async fn delete(&self, name: &str) -> Result<bool, String> {
        let inner = self.inner.lock();
        let table_name = Self::get_table_name(&*inner, name)?;

        if inner.opened.contains_key(&table_name) {
            return Err("Not deleting table that is still opened".to_owned());
        }
        let dbpath = Self::get_dbpath(&inner, &table_name)?;
        let ret = std::fs::remove_file(dbpath).is_ok();
        Ok(ret)
    }
}
