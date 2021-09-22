mod config;
mod engine;

mod grpc {
    tonic::include_proto!("grpc");
}

use async_once::AsyncOnce;
use bson::{doc, Document};
use engine::RusDbEngine;
use grpc::rus_db_server::{RusDb, RusDbServer};
use grpc::*;
use lazy_static::lazy_static;
use std::str::FromStr;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

lazy_static! {
    static ref ENGINE: AsyncOnce<Arc<RusDbEngine>> = AsyncOnce::new(async {
        let conf = config::load().await;
        RusDbEngine::create(&conf.engine).await
    });
}

#[derive(Debug, Default)]
pub struct RusDbServ;

impl RusDbServ {
    pub fn sanitize_collection(&self, name: &str) -> Option<String> {
        let colname = name.to_lowercase();
        let name_valid = !colname.contains(|c| match c {
            '.' | '/' | '\\' => true,
            _ => false,
        });
        if name_valid {
            Some(colname)
        } else {
            None
        }
    }
}

#[tonic::async_trait]
impl RusDb for RusDbServ {
    async fn insert(
        &self,
        request: Request<InsertRequest>,
    ) -> Result<Response<InsertResponses>, Status> {
        let req = request.get_ref();
        let name = self.sanitize_collection(&req.collection);
        let colname = {
            if name.is_none() {
                return Err(Status::invalid_argument(format!(
                    "Collection name contains invalid characters."
                )));
            } else {
                name.unwrap()
            }
        };
        if req.documents.len() == 0 {
            return Err(Status::invalid_argument(format!(
                "Documents field must contain at least one document."
            )));
        }
        let engine = ENGINE.get().await.clone();
        let mut responses: Vec<InsertResponse> = Vec::with_capacity(req.documents.len());
        if let Some(_col) = engine.get_collection(&colname).await {
            let mut col = _col.write().await;
            for data in &req.documents {
                match bson::from_slice::<Document>(data) {
                    Ok(mut doc) => {
                        if let Some(id) = doc.get("_id") {
                            if bson::from_bson::<Uuid>(id.clone()).is_err() {
                                let uid = Uuid::new_v4();
                                doc.insert("_id", uid);
                            }
                        } else {
                            let uid = Uuid::new_v4();
                            doc.insert("_id", bson::to_bson(&uid).unwrap());
                        }
                        let id = bson::from_bson::<Uuid>(doc.get("_id").unwrap().clone()).unwrap();
                        (*col).insert(id, doc.clone());
                        if req.return_old {
                            responses.push(InsertResponse {
                                id: id.to_string(),
                                document: Some(bson::to_vec(&doc).unwrap()),
                            })
                        } else {
                            responses.push(InsertResponse {
                                id: id.to_string(),
                                document: None,
                            });
                        }
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }
            Ok(Response::new(InsertResponses {
                count: responses.len() as u32,
                inserts: responses,
            }))
        } else {
            Err(Status::not_found(format!(
                "The collection could not be loaded."
            )))
        }
    }
    async fn update(
        &self,
        request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponses>, Status> {
        let req = request.get_ref();
        let name = self.sanitize_collection(&req.collection);
        let colname = {
            if name.is_none() {
                return Err(Status::invalid_argument(format!(
                    "Collection name contains invalid characters."
                )));
            } else {
                name.unwrap()
            }
        };
        let filter: Document = {
            let data = &req.filter;
            bson::from_slice(data).unwrap_or_default()
        };
        let updates: Document = {
            let data = &req.updates;
            bson::from_slice(data).unwrap_or_default()
        };
        if updates.len() == 0 {
            return Err(Status::invalid_argument("Updates document is empty."));
        }
        let engine = ENGINE.get().await.clone();
        if let Some(col) = engine.get_collection(&colname).await {
            let mut lock = col.write().await;
            let mut updated: Vec<Document> = Vec::with_capacity(req.limit.unwrap_or(10) as usize);
            for (_, v) in &mut *lock {
                let mut result = true;
                if filter.len() > 0 {
                    for (dk, dv) in &filter {
                        if let Some(fv) = v.get(dk) {
                            if !fv.eq(dv) {
                                result = false;
                                break;
                            }
                        } else {
                            result = false;
                            break;
                        }
                    }
                }
                if result {
                    for (dk, dv) in &updates {
                        if dk != "_id" {
                            v.insert(dk, dv.clone());
                        }
                    }
                    updated.push(v.clone());
                    if req.limit.is_some() {
                        if updated.len() == req.limit.unwrap() as usize {
                            break;
                        }
                    }
                }
            }
            Ok(Response::new(UpdateResponses {
                count: updated.len() as u32,
                updated: updated
                    .into_iter()
                    .map(|v| bson::to_vec(&v).unwrap())
                    .collect(),
            }))
        } else {
            Err(Status::internal("unable to find collection."))
        }
    }
    async fn remove(
        &self,
        request: Request<RemoveRequest>,
    ) -> Result<Response<RemoveResponse>, Status> {
        let req = request.get_ref();
        let name = self.sanitize_collection(&req.collection);
        let colname = {
            if name.is_none() {
                return Err(Status::invalid_argument(format!(
                    "Collection name contains invalid characters."
                )));
            } else {
                name.unwrap()
            }
        };
        let filter: Document = {
            let data = &req.filter;
            bson::from_slice(data).unwrap_or_default()
        };
        let engine = ENGINE.get().await.clone();
        if let Some(col) = engine.get_collection(&colname).await {
            let mut lock = col.write().await;
            let mut entries: Vec<Uuid> = Vec::with_capacity(req.limit.unwrap_or(10) as usize);
            for (k, v) in &*lock {
                let mut result = true;
                if filter.len() > 0 {
                    for (dk, dv) in &filter {
                        if let Some(bv) = v.get(dk) {
                            if !bv.eq(dv) {
                                result = false;
                                break;
                            }
                        } else {
                            result = false;
                            break;
                        }
                    }
                }
                if result {
                    entries.push(k.clone());
                    if req.limit.is_some() {
                        if req.limit.unwrap() as usize == entries.len() {
                            break;
                        }
                    }
                }
            }
            for uid in &entries {
                (*lock).remove(uid);
            }
            Ok(Response::new(RemoveResponse {
                count: entries.len() as u32,
            }))
        } else {
            Err(Status::internal("unable to find collection."))
        }
    }
    async fn find(&self, request: Request<FindRequest>) -> Result<Response<FindResponse>, Status> {
        let req = request.get_ref();
        let name = self.sanitize_collection(&req.collection);
        let colname = {
            if name.is_none() {
                return Err(Status::invalid_argument(format!(
                    "Collection name contains invalid characters."
                )));
            } else {
                name.unwrap()
            }
        };
        let filters: Document = {
            if let Some(data) = &req.filter {
                bson::from_slice(data).unwrap_or_default()
            } else {
                Document::default()
            }
        };
        let engine = ENGINE.get().await.clone();
        if let Some(col) = engine.get_collection(&colname).await {
            let mut res: Vec<Document> = Vec::with_capacity(req.limit.unwrap_or(10) as usize);
            let lock = col.read().await;
            for (_, doc) in &*lock {
                let mut result = true;
                if filters.len() > 0 {
                    for (k, v) in &filters {
                        if let Some(dv) = doc.get(k) {
                            if !v.eq(dv) {
                                result = false;
                                break;
                            }
                        } else {
                            result = false;
                            break;
                        }
                    }
                }
                if result {
                    res.push(doc.clone());
                    if let Some(limit) = req.limit {
                        if res.len() == limit as usize {
                            break;
                        }
                    }
                }
            }
            Ok(Response::new(FindResponse {
                count: res.len() as u32,
                documents: res.into_iter().map(|v| bson::to_vec(&v).unwrap()).collect(),
            }))
        } else {
            Err(Status::internal("unable to find collection."))
        }
    }
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.get_ref();
        let name = self.sanitize_collection(&req.collection);
        let colname = {
            if name.is_none() {
                return Err(Status::invalid_argument(format!(
                    "Collection name contains invalid characters."
                )));
            } else {
                name.unwrap()
            }
        };
        let engine = ENGINE.get().await.clone();
        if let Some(col) = engine.get_collection(&colname).await {
            if let Ok(uid) = Uuid::from_str(&req.id) {
                let lock = col.read().await;
                if let Some(doc) = (*lock).get(&uid) {
                    let data = bson::to_vec(doc).unwrap();
                    Ok(Response::new(GetResponse {
                        document: Some(data),
                    }))
                } else {
                    Ok(Response::new(GetResponse { document: None }))
                }
            } else {
                Err(Status::invalid_argument(format!(
                    "{} is not a valid Uuid",
                    &req.id
                )))
            }
        } else {
            Err(Status::internal("unable to find collection."))
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Loading configuration...");
    let conf = config::load().await;
    let _engine = ENGINE.get().await.clone();
    println!(
        "Starting gRPC server at {}:{}...",
        conf.grpc.ip, conf.grpc.port
    );
    let addr = format!("{}:{}", conf.grpc.ip, conf.grpc.port)
        .parse()
        .unwrap();
    let rusdb_server = RusDbServ::default();
    Server::builder()
        .add_service(RusDbServer::new(rusdb_server))
        .serve(addr)
        .await
        .unwrap();
}
