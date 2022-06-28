use crate::xx::*;
use crate::*;
use serde::{Deserialize, Serialize};

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use keyvaluedb_web::*;
    } else {
        use keyvaluedb_sqlite::*;
    }
}

pub struct TableDBInner {
    table: String,
    table_store: TableStore,
    database: Database,
}

impl Drop for TableDBInner {
    fn drop(&mut self) {
        self.table_store.on_table_db_drop(self.table.clone());
    }
}

#[derive(Clone)]
pub struct TableDB {
    inner: Arc<Mutex<TableDBInner>>,
}

impl TableDB {
    pub fn new(table: String, table_store: TableStore, database: Database) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TableDBInner {
                table,
                table_store,
                database,
            })),
        }
    }

    pub fn try_new_from_weak_inner(weak_inner: Weak<Mutex<TableDBInner>>) -> Option<Self> {
        weak_inner.upgrade().map(|table_db_inner| Self {
            inner: table_db_inner,
        })
    }

    pub fn weak_inner(&self) -> Weak<Mutex<TableDBInner>> {
        Arc::downgrade(&self.inner)
    }

    pub async fn get_column_count(&self) -> Result<u32, String> {
        let db = &self.inner.lock().database;
        db.num_columns()
            .map_err(|e| format!("failed to get column count: {}", e))
    }

    pub async fn get_keys(&self, col: u32) -> Result<Vec<Box<[u8]>>, String> {
        let db = &self.inner.lock().database;
        let mut out: Vec<Box<[u8]>> = Vec::new();
        db.iter(col, None, &mut |kv| {
            out.push(kv.0.clone().into_boxed_slice());
            Ok(true)
        })
        .map_err(|e| format!("failed to get keys for column {}: {}", col, e))?;
        Ok(out)
    }

    pub async fn store(&self, col: u32, key: &[u8], value: &[u8]) -> Result<(), String> {
        let db = &self.inner.lock().database;
        let mut dbt = db.transaction();
        dbt.put(col, key, value);
        db.write(dbt)
            .map_err(|e| format!("failed to store key {:?}: {}", key, e))
    }

    pub async fn store_cbor<T>(&self, col: u32, key: &[u8], value: &T) -> Result<(), String>
    where
        T: Serialize,
    {
        let v = serde_cbor::to_vec(value).map_err(|_| "couldn't store as CBOR".to_owned())?;

        let db = &self.inner.lock().database;
        let mut dbt = db.transaction();
        dbt.put(col, key, v.as_slice());
        db.write(dbt)
            .map_err(|e| format!("failed to store key {:?}: {}", key, e))
    }

    pub async fn load(&self, col: u32, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let db = &self.inner.lock().database;
        db.get(col, key)
            .map_err(|e| format!("failed to get key {:?}: {}", key, e))
    }

    pub async fn load_cbor<T>(&self, col: u32, key: &[u8]) -> Result<Option<T>, String>
    where
        T: for<'de> Deserialize<'de>,
    {
        let db = &self.inner.lock().database;
        let out = db
            .get(col, key)
            .map_err(|e| format!("failed to get key {:?}: {}", key, e))?;
        let b = match out {
            Some(v) => v,
            None => {
                return Ok(None);
            }
        };
        let obj = match serde_cbor::from_slice::<T>(&b) {
            Ok(value) => value,
            Err(e) => {
                return Err(format!("failed to deserialize: {}", e));
            }
        };
        Ok(Some(obj))
    }

    pub async fn delete(&self, col: u32, key: &[u8]) -> Result<bool, String> {
        let db = &self.inner.lock().database;
        let found = db
            .get(col, key)
            .map_err(|e| format!("failed to get key {:?}: {}", key, e))?;
        match found {
            None => Ok(false),
            Some(_) => {
                let mut dbt = db.transaction();
                dbt.delete(col, key);
                db.write(dbt)
                    .map_err(|e| format!("failed to delete key {:?}: {}", key, e))?;
                Ok(true)
            }
        }
    }
}
