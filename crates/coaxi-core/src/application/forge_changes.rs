//! Coaxi provider implementations, routed through the current Core workspace.
use super::{
    source::{self, ForgeProvider, auth::ResolvedAuth},
    *,
};

impl Core {
    pub(super) async fn forge_change_operation(
        &self,
        request: ForgeRequest,
        raw: Value,
        repository: Repository,
    ) -> Result<Value> {
        let provider = match repository.provider.as_str() {
            "github" => ForgeProvider::GitHub,
            "gitlab" => ForgeProvider::GitLab,
            _ => ForgeProvider::Gitea,
        };
        let auth = ResolvedAuth {
            provider,
            server_host: reqwest::Url::parse(&repository.web_base)?
                .host_str()
                .unwrap_or_default()
                .into(),
            api_base: repository.api_base.clone(),
            account_id: request.account_id.clone().unwrap_or_default(),
            username: String::new(),
            avatar_url: None,
            token: repository.token.clone(),
            scopes: Vec::new(),
        };
        let repo = &repository.owner_repo;
        let number = i64::try_from(request.number.unwrap_or(0))?;
        ensure!(
            request.action == "merge_options" || number > 0,
            "forge.item_number_required"
        );
        let write = request.action == "merge_pr";
        let _guard = if write {
            Some(MUTATIONS.lock().await)
        } else {
            None
        };
        let mut receipt = None;
        let mut identity = String::new();
        if write {
            ensure!(
                request.confirmed && !repository.token.is_empty(),
                "forge.confirmation_and_account_required"
            );
            ensure!(
                request.expected_repository_key.as_deref() == Some(repository.key().as_str()),
                "forge.repository_changed_refresh"
            );
            let sha = request
                .head_sha
                .as_deref()
                .context("forge.reviewed_head_required")?;
            ensure!(
                (40..=64).contains(&sha.len()) && sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "forge.invalid_head_sha"
            );
            ensure!(
                request.merge_method.is_some(),
                "forge.merge_method_required"
            );
            let id = request
                .command_id
                .as_deref()
                .context("forge.command_id_required")?;
            uuid::Uuid::parse_str(id)?;
            identity = crate::command::canonical_json_digest(
                &json!({"repository":repository.metadata(),"request":raw}),
            )?;
            let directory = self.data_dir.join("forge-operations");
            crate::platform::private_storage::prepare_private_directory(&directory)?;
            let file = directory.join(format!("{id}.json"));
            if file.exists() {
                let saved: Value = serde_json::from_slice(&fs::read(&file)?)?;
                ensure!(saved["identity"] == identity, "forge.command_conflict");
                ensure!(
                    saved["state"] == "finished",
                    "forge.write_unknown_check_remote"
                );
                return Ok(saved["result"].clone());
            }
            crate::platform::private_storage::atomic_write_private_json(
                &file,
                &json!({"identity":identity,"state":"started"}),
            )?;
            receipt = Some(file);
        }
        macro_rules! dispatch {
            ($function:ident $(,$argument:expr)*) => {match provider{
                ForgeProvider::GitHub=>source::github::$function(&auth,repo $(,$argument)*).await,
                ForgeProvider::GitLab=>source::gitlab::$function(&auth,repo $(,$argument)*).await,
                ForgeProvider::Gitea=>source::gitea::$function(&auth,repo $(,$argument)*).await,
            }};
        }
        let data = match request.action.as_str() {
            "change_detail" => serde_json::to_value(dispatch!(change_detail, number)?)?,
            "change_files" => {
                serde_json::to_value(dispatch!(list_change_files, number, request.page, 50)?)?
            }
            "merge_options" => serde_json::to_value(dispatch!(merge_options)?)?,
            "merge_pr" => serde_json::to_value(dispatch!(
                merge_change,
                number,
                request
                    .merge_method
                    .context("forge.merge_method_required")?,
                request.head_sha.as_deref()
            )?)?,
            _ => anyhow::bail!("forge.operation_not_admitted"),
        };
        let result = json!({"repository":repository.metadata(),"success":true,"status":200,"data":data,"nextPage":if request.action=="change_files"&&data["has_next"]==true{Some(request.page+1)}else{None}});
        if let Some(file) = receipt {
            crate::platform::private_storage::atomic_write_private_json(
                &file,
                &json!({"identity":identity,"state":"finished","result":result}),
            )?;
        }
        Ok(result)
    }
}
