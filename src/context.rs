
use mongodb::{options::ClientOptions, Client};



pub trait AsyncIOContext {
    fn is_debug(&self) -> bool;
}

//}


pub trait MongoDBContext {
    fn get_db(&self) -> mongodb::Database;
}

pub struct ThreadMotorContext {
    client: Client,
    database_name: String,
    debug: bool,
}

pub trait MaidContext: AsyncIOContext + MongoDBContext + Sync {}

impl ThreadMotorContext {
    pub async fn new(
        database_name: &str,
        host: &str,
        debug: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let options: ClientOptions = ClientOptions::parse_async(format!("{}", host)).await?;
        let client = Client::with_options(options)?;

        Ok(ThreadMotorContext {
            client: client,
            database_name: database_name.to_string(),
            debug: debug,
        })
    }
}

impl AsyncIOContext for ThreadMotorContext {
    fn is_debug(&self) -> bool {
        self.debug
    }
}

impl MongoDBContext for ThreadMotorContext {
    fn get_db(&self) -> mongodb::Database {
        self.client.database(&self.database_name)
    }
}

unsafe impl Sync for ThreadMotorContext {}

impl MaidContext for ThreadMotorContext {}
