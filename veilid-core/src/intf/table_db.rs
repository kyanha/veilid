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

    pub async fn get_column_count(&self) -> EyreResult<u32> {
        let db = &self.inner.lock().database;
        db.num_columns().wrap_err("failed to get column count: {}")
    }

    pub async fn get_keys(&self, col: u32) -> EyreResult<Vec<Box<[u8]>>> {
        let db = &self.inner.lock().database;
        let mut out: Vec<Box<[u8]>> = Vec::new();
        db.iter(col, None, &mut |kv| {
            out.push(kv.0.clone().into_boxed_slice());
            Ok(true)
        })
        .wrap_err("failed to get keys for column")?;
        Ok(out)
    }

    pub async fn store(&self, col: u32, key: &[u8], value: &[u8]) -> EyreResult<()> {
        let db = &self.inner.lock().database;
        let mut dbt = db.transaction();
        dbt.put(col, key, value);
        db.write(dbt).wrap_err("failed to store key")
    }

    pub async fn store_cbor<T>(&self, col: u32, key: &[u8], value: &T) -> EyreResult<()>
    where
        T: Serialize,
    {
        let v = serde_cbor::to_vec(value).wrap_err("couldn't store as CBOR")?;

        let db = &self.inner.lock().database;
        let mut dbt = db.transaction();
        dbt.put(col, key, v.as_slice());
        db.write(dbt).wrap_err("failed to store key")
    }

    pub async fn load(&self, col: u32, key: &[u8]) -> EyreResult<Option<Vec<u8>>> {
        let db = &self.inner.lock().database;
        db.get(col, key).wrap_err("failed to get key")
    }

    pub async fn load_cbor<T>(&self, col: u32, key: &[u8]) -> EyreResult<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let db = &self.inner.lock().database;
        let out = db.get(col, key).wrap_err("failed to get key")?;
        let b = match out {
            Some(v) => v,
            None => {
                return Ok(None);
            }
        };
        let obj = serde_cbor::from_slice::<T>(&b).wrap_err("failed to deserialize")?;
        Ok(Some(obj))
    }

    pub async fn delete(&self, col: u32, key: &[u8]) -> EyreResult<bool> {
        let db = &self.inner.lock().database;
        let found = db.get(col, key).wrap_err("failed to get key")?;
        match found {
            None => Ok(false),
            Some(_) => {
                let mut dbt = db.transaction();
                dbt.delete(col, key);
                db.write(dbt).wrap_err("failed to delete key")?;
                Ok(true)
            }
        }
    }
}
