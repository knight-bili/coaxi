//! 密钥及导入文件独立于通用操作入口；只使用已认证 User 的编辑端身份。
use super::*;
use axum::extract::Multipart;
use rovai_core::draft_client::DraftClient;

pub const MAX_ARCHIVE:usize=64*1024*1024;
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ArchiveInput{operation_id:String,source_digest:String,byte_size:u64,passphrase:String,manifest:Option<Value>}

async fn call(state:WebState,client:DraftClient,method:&'static str,input:Value)->Response{
    match state.core.request_for_editor(method,input,client).await{
        Ok(reply)=>Json(json!({"result":reply.result,"error":reply.error})).into_response(),
        Err(_)=>error(StatusCode::SERVICE_UNAVAILABLE,"configuration_result_unavailable"),
    }
}
pub async fn secret(State(state):State<WebState>,Extension(session):Extension<Arc<Session>>,body:std::result::Result<Json<Value>,axum::extract::rejection::JsonRejection>)->Response{
    let Ok(Json(input))=body else{return error(StatusCode::BAD_REQUEST,"configuration_input_invalid")};
    let Ok(_permit)=state.requests.clone().try_acquire_owned()else{return error(StatusCode::TOO_MANY_REQUESTS,"request_capacity")};
    if !state.sessions.is_live(&session){return error(StatusCode::UNAUTHORIZED,"session_required")}
    let client=DraftClient::verified_web(&session.client_id).expect("Host editor identity");
    call(state,client,"configuration.secrets.prepare",input).await
}
pub async fn receipt(State(state):State<WebState>,Extension(session):Extension<Arc<Session>>,body:std::result::Result<Json<Value>,axum::extract::rejection::JsonRejection>)->Response{
    let Ok(Json(input))=body else{return error(StatusCode::BAD_REQUEST,"configuration_input_invalid")};
    let Ok(_permit)=state.requests.clone().try_acquire_owned()else{return error(StatusCode::TOO_MANY_REQUESTS,"request_capacity")};
    if !state.sessions.is_live(&session){return error(StatusCode::UNAUTHORIZED,"session_required")}
    let client=DraftClient::verified_web(&session.client_id).expect("Host editor identity");
    call(state,client,"configuration.archive.receipt",input).await
}
pub async fn archive(State(state):State<WebState>,Extension(session):Extension<Arc<Session>>,mut form:Multipart)->Response{
    let Ok(permit)=state.uploads.clone().try_acquire_owned()else{return error(StatusCode::TOO_MANY_REQUESTS,"upload_capacity")};
    let mut metadata=None;let mut upload=None;
    loop{
        let field=match form.next_field().await{Ok(Some(field))=>field,Ok(None)=>break,Err(_)=>return error(StatusCode::BAD_REQUEST,"configuration_upload_invalid")};
        match field.name(){
            Some("metadata")if metadata.is_none()=>{
                let Ok(bytes)=field.bytes().await else{return error(StatusCode::BAD_REQUEST,"configuration_upload_invalid")};
                if bytes.len()>128*1024{return error(StatusCode::PAYLOAD_TOO_LARGE,"configuration_metadata_limit")}
                metadata=serde_json::from_slice::<ArchiveInput>(&bytes).ok();
                if metadata.is_none(){return error(StatusCode::BAD_REQUEST,"configuration_upload_invalid")}
            },
            Some("file")if upload.is_none()=>match uploads::receive_bounded(field,MAX_ARCHIVE).await{Ok(value)=>upload=Some(value),Err(_)=>return error(StatusCode::BAD_REQUEST,"configuration_upload_invalid")},
            _=>return error(StatusCode::BAD_REQUEST,"configuration_upload_invalid"),
        }
    }
    let(Some(input),Some(contents))=(metadata,upload)else{return error(StatusCode::BAD_REQUEST,"configuration_upload_invalid")};
    if input.byte_size!=contents.size||input.source_digest!=format!("sha256:{}",contents.hash)||input.passphrase.len()>16000||!state.sessions.is_live(&session){return error(StatusCode::BAD_REQUEST,"configuration_upload_changed")}
    let client=DraftClient::verified_web(&session.client_id).expect("Host editor identity");
    // 接收完后由独立任务持有私有临时文件；HTTP 断线不删除仍被 Core 读取的输入。
    let pending=tokio::spawn(async move{
        let _permit=permit;
        let value=json!({"operationId":input.operation_id,"path":contents.temporary.path(),"sourceDigest":input.source_digest,"byteSize":input.byte_size,"passphrase":input.passphrase,"manifest":input.manifest});
        let result=state.core.request_for_editor("configuration.archive.registerLocal",value,client).await;
        drop(contents);result
    });
    match pending.await{
        Ok(Ok(reply))=>Json(json!({"result":reply.result,"error":reply.error})).into_response(),
        _=>error(StatusCode::SERVICE_UNAVAILABLE,"configuration_result_unavailable"),
    }
}
