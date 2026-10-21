//! Codeg forge REST/provider rules (aff1e6c, Apache-2.0), with Rovai scope,
//! credentials and command receipts. Repository identity is derived from Git.
use super::*;
use super::workbench::{Scope, MUTATIONS};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::{fs, time::Duration};
#[path="forge_envelope.rs"]
mod envelope;

#[derive(Deserialize, Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Account { id:String, provider:String, base_url:String, token:String }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct AccountInput { id:String, provider:String, base_url:String, token:Option<String>, #[serde(default)] remove:bool }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ForgeRequest {
    scope:Scope,
    action:String,
    #[serde(default)] account_id:Option<String>,
    #[serde(default="origin")] remote:String,
    #[serde(default)] number:Option<u64>,
    #[serde(default="first_page")] page:u32,
    #[serde(default)] state:Option<String>,
    #[serde(default)] title:Option<String>,
    #[serde(default)] body:Option<String>,
    #[serde(default)] head:Option<String>,
    #[serde(default)] base:Option<String>,
    #[serde(default)] draft:bool,
    #[serde(default)] command_id:Option<String>,
    #[serde(default)] confirmed:bool,
}
fn origin()->String{"origin".into()}
fn first_page()->u32{1}

fn public_accounts(accounts:&[Account])->Value {
    json!({"accounts":accounts.iter().map(|a|json!({"id":a.id,"provider":a.provider,"baseUrl":a.base_url,"hasToken":!a.token.is_empty()})).collect::<Vec<_>>()})
}

struct Repository { provider:String, web_base:String, api_base:String, owner_repo:String, token:String }
impl Repository {
    fn metadata(&self)->Value{json!({"provider":self.provider,"webUrl":format!("{}/{}",self.web_base,self.owner_repo),"ownerRepo":self.owner_repo})}
    fn endpoint(&self, collection:&str, number:Option<u64>, comments:bool)->Result<reqwest::Url>{
        let mut url=reqwest::Url::parse(&self.api_base)?;
        {let mut parts=url.path_segments_mut().map_err(|_|anyhow::anyhow!("forge.invalid_api_base"))?;parts.pop_if_empty();
            if self.provider=="gitlab"{parts.push("projects").push(&self.owner_repo);}else{parts.push("repos");for part in self.owner_repo.split('/'){parts.push(part);}}
            parts.push(if self.provider=="gitlab"&&collection=="pulls"{"merge_requests"}else{collection});
            if let Some(number)=number{parts.push(&number.to_string());}
            if comments{parts.push(if self.provider=="gitlab"{"notes"}else{"comments"});}
        }
        Ok(url)
    }
}

impl Core {
    pub(super) async fn forge_accounts(&self, method:&str, value:Value)->Result<Value>{
        let _write=MUTATIONS.lock().await;
        let path=self.data_dir.join("forge-accounts.json");
        let mut accounts:Vec<Account>=if path.exists(){serde_json::from_slice(&fs::read(&path)?)?}else{Vec::new()};
        if method=="forge.accounts.get"{return Ok(public_accounts(&accounts))}
        ensure!(method=="forge.accounts.set","forge.operation_not_admitted");
        let input:AccountInput=serde_json::from_value(value)?;
        uuid::Uuid::parse_str(&input.id).context("forge.invalid_account_id")?;
        if input.remove{accounts.retain(|a|a.id!=input.id);}else{
            ensure!(["github","gitlab","gitea","forgejo"].contains(&input.provider.as_str()),"forge.unsupported_provider");
            let mut base=reqwest::Url::parse(&input.base_url)?;
            ensure!(["http","https"].contains(&base.scheme())&&base.host_str().is_some()&&base.username().is_empty()&&base.password().is_none(),"forge.invalid_base_url");
            base.set_query(None);base.set_fragment(None);
            let previous=accounts.iter().find(|a|a.id==input.id);
            let token=input.token.or_else(||previous.map(|a|a.token.clone())).unwrap_or_default();
            ensure!(!token.contains(['\0','\r','\n']),"forge.invalid_credential");
            accounts.retain(|a|a.id!=input.id);
            accounts.push(Account{id:input.id,provider:input.provider,base_url:base.as_str().trim_end_matches('/').into(),token});
        }
        crate::platform::private_storage::atomic_write_private_json(&path,&accounts)?;
        Ok(public_accounts(&accounts))
    }

    async fn forge_repository(&self, request:&ForgeRequest)->Result<Repository>{
        let root=self.workbench_root(&request.scope).await?;
        ensure!(!request.remote.is_empty()&&!request.remote.starts_with('-')&&!request.remote.contains(['\0','\r','\n']),"forge.invalid_remote");
        let output=self.mission_git().await?.workbench_output(&root,&["remote".into(),"get-url".into(),request.remote.clone()]).await?;
        ensure!(output.status.success(),"forge.remote_unavailable");
        let remote=String::from_utf8(output.stdout.bytes)?;
        let remote=remote.trim();
        let parsed=if remote.contains("://"){reqwest::Url::parse(remote)?}else{
            let (host,path)=remote.split_once(':').context("forge.unsupported_remote")?;
            reqwest::Url::parse(&format!("ssh://{}/{}",host,path))?
        };
        let host=parsed.host_str().context("forge.invalid_remote")?;
        let path=self.data_dir.join("forge-accounts.json");
        let accounts:Vec<Account>=if path.exists(){serde_json::from_slice(&fs::read(path)?)?}else{Vec::new()};
        let account=accounts.into_iter().find(|a|request.account_id.as_ref().is_none_or(|id|id==&a.id)&&reqwest::Url::parse(&a.base_url).ok().is_some_and(|base|base.host_str().is_some_and(|h|h.eq_ignore_ascii_case(host))));
        let account=match account{
            Some(account)=>account,
            None=>{ensure!(request.account_id.is_none(),"forge.account_unavailable");Account{id:String::new(),provider:if host.eq_ignore_ascii_case("github.com"){"github"}else if host.eq_ignore_ascii_case("gitlab.com"){"gitlab"}else{anyhow::bail!("forge.account_required")}.into(),base_url:format!("https://{host}"),token:String::new()}}
        };
        let web=reqwest::Url::parse(&account.base_url)?;
        let prefix=web.path().trim_matches('/');
        let raw=parsed.path().trim_start_matches('/');
        let owner_repo=if prefix.is_empty(){raw}else{raw.strip_prefix(&format!("{prefix}/")).context("forge.remote_base_mismatch")?}.trim_end_matches('/').strip_suffix(".git").unwrap_or(if prefix.is_empty(){raw}else{raw.strip_prefix(&format!("{prefix}/")).unwrap_or(raw)}).trim_end_matches('/').to_owned();
        ensure!(owner_repo.split('/').count()>=2&&!owner_repo.split('/').any(|s|s.is_empty()||s=="."||s==".."),"forge.invalid_repository");
        if account.provider!="gitlab"{ensure!(owner_repo.split('/').count()==2,"forge.invalid_repository");}
        let api_base=match account.provider.as_str(){
            "github" if host.eq_ignore_ascii_case("github.com")=>"https://api.github.com".into(),
            "github"=>format!("{}/api/v3",account.base_url),
            "gitlab"=>format!("{}/api/v4",account.base_url),
            _=>format!("{}/api/v1",account.base_url),
        };
        Ok(Repository{provider:account.provider,web_base:account.base_url,api_base,owner_repo,token:account.token})
    }

    pub(super) async fn forge_operation(&self,value:Value)->Result<Value>{
        let request:ForgeRequest=serde_json::from_value(value.clone())?;
        ensure!(request.page>0&&request.page<=100_000,"forge.invalid_page");
        let repository=self.forge_repository(&request).await?;
        if request.action=="repository"{return Ok(repository.metadata())}
        let write=matches!(request.action.as_str(),"create_issue"|"update_issue"|"create_pr"|"update_pr"|"comment_issue"|"comment_pr");
        let pull=request.action.ends_with("pr")||request.action=="pulls";
        let comments=request.action.starts_with("comment_")||request.action.starts_with("comments_");
        let mission_draft=request.action.starts_with("mission_draft_");
        ensure!(write||["issues","pulls","issue","pr","comments_issue","comments_pr","mission_draft_issue","mission_draft_pr"].contains(&request.action.as_str()),"forge.operation_not_admitted");
        let single=matches!(request.action.as_str(),"issue"|"pr"|"update_issue"|"update_pr")||comments||mission_draft;
        if single{ensure!(request.number.is_some_and(|n|n>0),"forge.item_number_required");}
        let collection=if pull && !(comments&&repository.provider!="gitlab"){"pulls"}else{"issues"};
        let mut url=repository.endpoint(collection,if single{request.number}else{None},comments)?;
        if !write{url.query_pairs_mut().append_pair("page",&request.page.to_string()).append_pair("per_page","50");if let Some(state)=&request.state{ensure!(["open","closed","all","opened","merged"].contains(&state.as_str()),"forge.invalid_state");url.query_pairs_mut().append_pair("state",if repository.provider=="gitlab"&&state=="open"{"opened"}else{state});}}
        let mut body=serde_json::Map::new();
        if let Some(title)=&request.title{body.insert("title".into(),json!(title));}
        if let Some(text)=&request.body{body.insert(if repository.provider=="gitlab"&&!comments{"description"}else{"body"}.into(),json!(text));}
        if let Some(state)=&request.state{if repository.provider=="gitlab"{ensure!(["open","closed"].contains(&state.as_str()),"forge.invalid_state");body.insert("state_event".into(),json!(if state=="open"{"reopen"}else{"close"}));}else{body.insert("state".into(),json!(state));}}
        if request.action=="create_pr"{
            let head=request.head.as_deref().filter(|s|!s.trim().is_empty()).context("forge.head_required")?;
            let base=request.base.as_deref().filter(|s|!s.trim().is_empty()).context("forge.base_required")?;
            body.insert(if repository.provider=="gitlab"{"source_branch"}else{"head"}.into(),json!(head));body.insert(if repository.provider=="gitlab"{"target_branch"}else{"base"}.into(),json!(base));
            if repository.provider!="gitlab"{body.insert("draft".into(),json!(request.draft));}
        }
        let _write=if write{Some(MUTATIONS.lock().await)}else{None};
        let mut receipt=None;
        let mut identity=String::new();
        if write{
            ensure!(request.confirmed&&!repository.token.is_empty(),"forge.confirmation_and_account_required");
            let id=request.command_id.as_deref().context("forge.command_id_required")?;uuid::Uuid::parse_str(id)?;
            identity=crate::command::canonical_json_digest(&json!({"repository":repository.metadata(),"request":value}))?;
            let directory=self.data_dir.join("forge-operations");crate::platform::private_storage::prepare_private_directory(&directory)?;
            let file=directory.join(format!("{id}.json"));
            if file.exists(){let saved:Value=serde_json::from_slice(&fs::read(&file)?)?;ensure!(saved["identity"]==identity,"forge.command_conflict");ensure!(saved["state"]=="finished","forge.write_unknown_check_remote");return Ok(saved["result"].clone())}
            crate::platform::private_storage::atomic_write_private_json(&file,&json!({"identity":identity,"state":"started"}))?;receipt=Some(file);
        }
        let client=reqwest::Client::builder().timeout(Duration::from_secs(45)).redirect(reqwest::redirect::Policy::none()).user_agent("Codeg-Rovai-Workbench").build()?;
        let method=if !write{reqwest::Method::GET}else if request.action.starts_with("update_"){if repository.provider=="gitlab"{reqwest::Method::PUT}else{reqwest::Method::PATCH}}else{reqwest::Method::POST};
        let mut call=client.request(method,url).header("Accept","application/json");
        if !repository.token.is_empty(){call=if repository.provider=="gitlab"{call.header("PRIVATE-TOKEN",&repository.token)}else if matches!(repository.provider.as_str(),"gitea"|"forgejo"){call.header("Authorization",format!("token {}",repository.token))}else{call.bearer_auth(&repository.token)};}
        if write{call=call.json(&body);}
        let mut response=call.send().await.context("forge.network_unavailable")?;
        let status=response.status();
        let next=response.headers().get("x-next-page").and_then(|v|v.to_str().ok()).is_some_and(|v|!v.is_empty())||response.headers().get("link").and_then(|v|v.to_str().ok()).is_some_and(|v|v.contains("rel=\"next\""));
        let mut bytes=Vec::new();while let Some(chunk)=response.chunk().await?{ensure!(bytes.len()+chunk.len()<=16*1024*1024,"forge.response_too_large");bytes.extend_from_slice(&chunk);}
        let data=if status.is_success()&&!bytes.is_empty(){serde_json::from_slice::<Value>(&bytes).context("forge.invalid_response")?}else{Value::Null};
        let draft=if mission_draft&&status.is_success(){
            let number=request.number.context("forge.item_number_required")?;
            let route=if repository.provider=="gitlab"{if pull{"-/merge_requests"}else{"-/issues"}}else if pull{"pull"}else{"issues"};
            let url=format!("{}/{}/{route}/{number}",repository.web_base,repository.owner_repo);
            let title=data["title"].as_str().unwrap_or_default().to_owned();
            let snapshot=envelope::ForgeSnapshot{title:title.clone(),body:data["body"].as_str().or_else(||data["description"].as_str()).map(String::from),labels:Vec::new(),author:data.pointer("/user/login").or_else(||data.pointer("/author/username")).and_then(Value::as_str).map(String::from)};
            Some(json!({"title":format!("#{} {}",number,title),"description":format!("来源：{}\n{}",url,envelope::forge_untrusted_envelope(&repository.provider,&snapshot)),"sourceUrl":url,"sourceKey":format!("{}:{}:{}:{}:{}",repository.provider,repository.web_base,repository.owner_repo,if pull{"pr"}else{"issue"},number)}))
        }else{None};
        let result=json!({"repository":repository.metadata(),"success":status.is_success(),"status":status.as_u16(),"data":data,"missionDraft":draft,"nextPage":if next{Some(request.page+1)}else{None}});
        if let Some(receipt)=receipt{crate::platform::private_storage::atomic_write_private_json(&receipt,&json!({"identity":identity,"state":"finished","result":result}))?;}
        Ok(result)
    }
}
