use crate::intf::table_db::*;
use crate::intf::*;
use crate::*;
use keyvaluedb_web::*;

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
            self.inner.lock().opened.len() == 0,
            "all open databases should have been closed"
        );
    }

    pub fn on_table_db_drop(&self, table: String) {
        let mut inner = self.inner.lock();
        match inner.opened.remove(&table) {
            Some(_) => (),
            None => {
                assert!(false, "should have removed an item");
            }
        }
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
        Ok(if namespace.len() == 0 {
            format!("{}", table)
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
        let db = Database::open(table_name.clone(), column_count)
            .await
            .map_err(|e| format!("failed to open tabledb at: {} ({})", table_name, e))?;
        info!("opened table store '{}' with table name '{:?}' with {} columns", name, table_name, column_count);

        let table_db = TableDB::new(table_name.clone(), self.clone(), db);

        inner.opened.insert(table_name, table_db.weak_inner());

        Ok(table_db)
    }

    pub async fn delete(&self, name: &str) -> Result<bool, String> {
        trace!("TableStore::delete {}", name);
        let inner = self.inner.lock();
        let table_name = Self::get_table_name(&*inner, name)?;

        if inner.opened.contains_key(&table_name) {
            trace!(
                "TableStore::delete {}: Not deleting, still open.",
                table_name
            );
            return Err("Not deleting table that is still opened".to_owned());
        }

        if utils::is_nodejs() {
            Err("unimplemented".to_owned())
        } else if utils::is_browser() {
            let out = match Database::delete(table_name.clone()).await {
                Ok(_) => true,
                Err(_) => false,
            };
            //.map_err(|e| format!("failed to delete tabledb at: {} ({})", table_name, e))?;
            trace!("TableStore::deleted {}", table_name);
            Ok(out)
        } else {
            Err("unimplemented".to_owned())
        }
    }
}
