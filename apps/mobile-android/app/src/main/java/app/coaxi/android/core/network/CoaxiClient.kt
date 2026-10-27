package app.coaxi.android.core.network

import app.coaxi.android.core.model.AgentType
import app.coaxi.android.core.model.ConversationConnectionInfo
import app.coaxi.android.core.model.ConversationDetail
import app.coaxi.android.core.model.ConversationIdBody
import app.coaxi.android.core.model.ConnectBody
import app.coaxi.android.core.model.ConnectionIdBody
import app.coaxi.android.core.model.AnswerPlanApprovalBody
import app.coaxi.android.core.model.AnswerQuestionBody
import app.coaxi.android.core.model.PlanApprovalAnswer
import app.coaxi.android.core.model.PlanApprovalDecision
import app.coaxi.android.core.model.CreateConversationBody
import app.coaxi.android.core.model.EmptyBody
import app.coaxi.android.core.model.FetchKimiModelsBody
import app.coaxi.android.core.model.FindConnectionBody
import app.coaxi.android.core.model.PiCommandValidation
import app.coaxi.android.core.model.PiConfigProjection
import app.coaxi.android.core.model.TaskIdBody
import app.coaxi.android.core.model.UpdateKimiCodeConfigBody
import app.coaxi.android.core.model.UpdatePiConfigBody
import app.coaxi.android.core.model.ValidatePiCommandBody
import app.coaxi.android.core.model.FolderDetail
import app.coaxi.android.core.model.HealthResponse
import app.coaxi.android.core.model.ListConversationsBody
import app.coaxi.android.core.model.PromptBody
import app.coaxi.android.core.model.PromptInputBlock
import app.coaxi.android.core.model.QuestionAnswer
import app.coaxi.android.core.model.RespondPermissionBody
import app.coaxi.android.core.model.ConversationSummary
import app.coaxi.android.core.model.UpdateConversationPinnedBody
import app.coaxi.android.core.model.UpdateConversationStatusBody
import app.coaxi.android.core.model.UpdateConversationTitleBody
import app.coaxi.android.core.model.CloneRepositoryBody
import app.coaxi.android.core.model.DirectoryEntry
import app.coaxi.android.core.model.DirectoryItem
import app.coaxi.android.core.model.FilePreviewContent
import app.coaxi.android.core.model.AccountTokenBody
import app.coaxi.android.core.model.GitAddFilesBody
import app.coaxi.android.core.model.GitBranchList
import app.coaxi.android.core.model.GitCheckoutBody
import app.coaxi.android.core.model.GitCommitBody
import app.coaxi.android.core.model.GitCommitResult
import app.coaxi.android.core.model.DeleteFileTreeEntryBody
import app.coaxi.android.core.model.GitCredentials
import app.coaxi.android.core.model.GitDiffBody
import app.coaxi.android.core.model.GitFetchBody
import app.coaxi.android.core.model.GitLogBody
import app.coaxi.android.core.model.GitLogResult
import app.coaxi.android.core.model.GitNewBranchBody
import app.coaxi.android.core.model.GitPullBody
import app.coaxi.android.core.model.GitPullResult
import app.coaxi.android.core.model.GitPushBody
import app.coaxi.android.core.model.GitPushInfo
import app.coaxi.android.core.model.GitPushResult
import app.coaxi.android.core.model.GitRemote
import app.coaxi.android.core.model.GitShowDiffBody
import app.coaxi.android.core.model.GitStatusBody
import app.coaxi.android.core.model.GitStatusEntry
import app.coaxi.android.core.model.PathFileBody
import app.coaxi.android.core.model.OpenWorktreeFolderBody
import app.coaxi.android.core.model.PathBody
import app.coaxi.android.core.model.ReadFilePreviewBody
import app.coaxi.android.core.model.ResolveWorktreeFolderBody
import app.coaxi.android.core.model.WorktreeResolution
import app.coaxi.android.core.model.EnabledFlag
import app.coaxi.android.core.model.EnabledSettingsBody
import app.coaxi.android.core.model.IdBody
import app.coaxi.android.core.model.ModelProviderInfo
import app.coaxi.android.core.model.CreateModelProviderBody
import app.coaxi.android.core.model.UpdateModelProviderBody
import app.coaxi.android.core.model.QuickMessage
import app.coaxi.android.core.model.QuickMessageCreateBody
import app.coaxi.android.core.model.QuickMessageReorderBody
import app.coaxi.android.core.model.QuickMessageUpdateBody
import app.coaxi.android.core.model.AcpAgentInfo
import app.coaxi.android.core.model.CursorAuthStatus
import app.coaxi.android.core.model.CursorModelsResult
import app.coaxi.android.core.model.CursorStructuredConfig
import app.coaxi.android.core.model.AppUpdateCheckResult
import app.coaxi.android.core.model.GitDetectResult
import app.coaxi.android.core.model.GitHubAccount
import app.coaxi.android.core.model.GitHubAccountList
import app.coaxi.android.core.model.GitHubTokenValidation
import app.coaxi.android.core.model.GitSettings
import app.coaxi.android.core.model.ValidateGitHubTokenBody
import app.coaxi.android.core.model.LocalMcpServer
import app.coaxi.android.core.model.SystemProxySettings
import app.coaxi.android.core.model.UpdateAgentEnvBody
import app.coaxi.android.core.model.AgentSkillContent
import app.coaxi.android.core.model.AgentSkillsListResult
import app.coaxi.android.core.model.SkillListBody
import app.coaxi.android.core.model.SkillReadBody
import app.coaxi.android.core.model.SkillSaveBody
import app.coaxi.android.core.model.SkillDeleteBody
import app.coaxi.android.core.model.ExpertListItem
import app.coaxi.android.core.model.ExpertInstallStatus
import app.coaxi.android.core.model.ExpertAgentBody
import app.coaxi.android.core.model.ExpertLinkBody
import app.coaxi.android.core.model.ExpertIdBody
import app.coaxi.android.core.model.ChatChannelInfo
import app.coaxi.android.core.model.ChannelStatusInfo
import app.coaxi.android.core.model.ChatChannelMessageLog
import app.coaxi.android.core.model.ChannelType
import app.coaxi.android.core.model.WebhookConfig
import app.coaxi.android.core.model.WeixinQrcode
import app.coaxi.android.core.model.WeixinQrStatus
import app.coaxi.android.core.model.CreateChatChannelBody
import app.coaxi.android.core.model.UpdateChatChannelBody
import app.coaxi.android.core.model.FieldEdit
import app.coaxi.android.core.model.ChannelTokenBody
import app.coaxi.android.core.model.ChannelIdOnlyBody
import app.coaxi.android.core.model.ListChannelMessagesBody
import app.coaxi.android.core.model.WeixinCheckBody
import app.coaxi.android.core.model.ChatPrefixBody
import app.coaxi.android.core.model.ChatLanguageBody
import app.coaxi.android.core.model.ChatWebhooksBody
import app.coaxi.android.core.model.SessionSnapshot
import app.coaxi.android.core.model.AgentOptionsSnapshot
import app.coaxi.android.core.model.DescribeAgentOptionsBody
import app.coaxi.android.core.model.SetModeBody
import app.coaxi.android.core.model.SetConfigOptionBody
import io.ktor.client.HttpClient
import io.ktor.client.request.header
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsText
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.contentType
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.putJsonObject

/**
 * HTTP client for a single coaxi server. Cheap to create per selected server;
 * holds the resolved base URL + bearer token and shares the injected Ktor
 * [HttpClient]. Every endpoint is `POST /api/<name>` with a JSON body
 * (camelCase) and a (usually snake_case) JSON response — see [CoaxiJson].
 *
 * Faithful port of the iOS `CoaxiClient`.
 */
class CoaxiClient(
    rawBaseUrl: String,
    private val token: String,
    private val http: HttpClient,
) {
    /** Base URL with any trailing slash trimmed; `/api/<name>` is appended. */
    val baseUrl: String = normalizeBaseUrl(rawBaseUrl)

    // region Endpoints

    /** Validate connectivity + auth for a server profile. */
    suspend fun health(): HealthResponse =
        decode(send("health", encode(EmptyBody)), HealthResponse.serializer())

    /** All projects/folders known to the server. */
    suspend fun listFolders(): List<FolderDetail> =
        decode(send("list_all_folder_details", encode(EmptyBody)), ListSerializer(FolderDetail.serializer()))

    /** Open (non-chat) folders only — the new-session folder picker's source. */
    suspend fun listOpenFolders(): List<FolderDetail> =
        decode(send("list_open_folder_details", encode(EmptyBody)), ListSerializer(FolderDetail.serializer()))

    /** Conversations, optionally filtered by folder / status / search text. */
    suspend fun listConversations(
        folderIds: List<Int>? = null,
        status: String? = null,
        search: String? = null,
        sortBy: String? = null,
    ): List<ConversationSummary> =
        decode(
            send(
                "list_all_conversations",
                encode(
                    ListConversationsBody(
                        folderIds = folderIds,
                        search = search,
                        sortBy = sortBy,
                        status = status,
                    ),
                ),
            ),
            ListSerializer(ConversationSummary.serializer()),
        )

    /** Full session detail incl. message history. */
    suspend fun conversationDetail(id: Int): ConversationDetail =
        decode(
            send("get_folder_conversation", encode(ConversationIdBody(id))),
            ConversationDetail.serializer(),
        )

    /** Create a conversation row up front and return its id (a bare JSON int). */
    suspend fun createConversation(folderId: Int, agentType: AgentType, title: String?): Int =
        decode(
            send("create_conversation", encode(CreateConversationBody(folderId, agentType, title))),
            Int.serializer(),
        )

    /**
     * Spawn (or resume, when [sessionId] is set) an agent process. Returns the
     * connection id (a bare JSON string).
     */
    suspend fun connect(
        agentType: AgentType,
        workingDir: String?,
        sessionId: String?,
        preferredModeId: String? = null,
        preferredConfigValues: Map<String, String>? = null,
    ): String {
        val text = send(
            "acp_connect",
            encode(ConnectBody(agentType, workingDir, sessionId, preferredModeId, preferredConfigValues)),
        )
        return decodeConnectionId(text)
    }

    /** Find a live connection already bound to a conversation, if any. */
    suspend fun findConnection(
        conversationId: Int,
        sessionId: String?,
        agentType: AgentType,
    ): ConversationConnectionInfo? {
        val text = send(
            "acp_find_connection_for_conversation",
            encode(FindConnectionBody(conversationId, sessionId, agentType)),
        )
        if (isJsonNull(text)) return null
        return decode(text, ConversationConnectionInfo.serializer())
    }

    /** Send a prompt to a live connection. The reply streams over the WebSocket. */
    suspend fun prompt(
        connectionId: String,
        blocks: List<PromptInputBlock>,
        folderId: Int?,
        conversationId: Int?,
        clientMessageId: String?,
    ) {
        send("acp_prompt", encode(PromptBody(connectionId, blocks, folderId, conversationId, clientMessageId)))
    }

    /** Cancel the in-flight turn on a connection. */
    suspend fun cancel(connectionId: String) {
        send("acp_cancel", encode(ConnectionIdBody(connectionId)))
    }

    /** Resolve a `permission_request` (or ExitPlanMode) by picking an option. */
    suspend fun respondPermission(connectionId: String, requestId: String, optionId: String) {
        send("acp_respond_permission", encode(RespondPermissionBody(connectionId, requestId, optionId)))
    }

    /** Answer an `ask_user_question`. Pass [QuestionAnswer.dismissed] to decline. */
    suspend fun answerQuestion(connectionId: String, questionId: String, answer: QuestionAnswer) {
        send("acp_answer_question", encode(AnswerQuestionBody(connectionId, questionId, answer)))
    }

    /**
     * Resolve Grok's blocked `exit_plan_mode`. The backend broadcasts
     * `plan_approval_resolved` so every client viewing the conversation clears its
     * card, then unblocks the parked ext request.
     */
    suspend fun answerPlanApproval(
        connectionId: String,
        approvalId: String,
        decision: PlanApprovalDecision,
        feedback: String?,
    ) {
        send(
            "acp_answer_plan_approval",
            encode(AnswerPlanApprovalBody(connectionId, approvalId, PlanApprovalAnswer(decision, feedback))),
        )
    }

    /** Pin or unpin a conversation (server `update_conversation_pinned`). */
    suspend fun setPinned(conversationId: Int, pinned: Boolean) {
        send("update_conversation_pinned", encode(UpdateConversationPinnedBody(conversationId, pinned)))
    }

    /** Rename a conversation (server `update_conversation_title`). */
    suspend fun renameConversation(conversationId: Int, title: String) {
        send("update_conversation_title", encode(UpdateConversationTitleBody(conversationId, title)))
    }

    /** Set a conversation's lifecycle status (server `update_conversation_status`). */
    suspend fun setStatus(conversationId: Int, status: String) {
        send("update_conversation_status", encode(UpdateConversationStatusBody(conversationId, status)))
    }

    /** Permanently delete a conversation (server `delete_conversation`). */
    suspend fun deleteConversation(conversationId: Int) {
        send("delete_conversation", encode(ConversationIdBody(conversationId)))
    }

    // endregion

    // region Projects / Git / filesystem

    /** Register/open a folder by absolute path; returns the upserted [FolderDetail]. */
    suspend fun openFolder(path: String): FolderDetail =
        decode(send("open_folder", encode(PathBody(path))), FolderDetail.serializer())

    /** The server's home directory (bare JSON string). */
    suspend fun getHomeDirectory(): String =
        decodeConnectionId(send("get_home_directory", encode(EmptyBody)))

    /** Directories under [path] (browser; dirs only). */
    suspend fun listDirectoryEntries(path: String): List<DirectoryEntry> =
        decode(send("list_directory_entries", encode(PathBody(path))), ListSerializer(DirectoryEntry.serializer()))

    /** Files + directories under [path] (file browser; camelCase wire). */
    suspend fun listDirectoryWithFiles(path: String): List<DirectoryItem> =
        decodeCamel(send("list_directory_with_files", encode(PathBody(path))), ListSerializer(DirectoryItem.serializer()))

    /** Read a file's text (path is relative to [rootPath]). */
    suspend fun readFilePreview(rootPath: String, path: String): FilePreviewContent =
        decode(send("read_file_preview", encode(ReadFilePreviewBody(rootPath, path))), FilePreviewContent.serializer())

    /** Working-tree changes. Throws [ApiError.Server] with code `not_a_git_repository` if not a repo. */
    suspend fun gitStatus(path: String, showAllUntracked: Boolean = true): List<GitStatusEntry> =
        decode(send("git_status", encode(GitStatusBody(path, showAllUntracked))), ListSerializer(GitStatusEntry.serializer()))

    /** Commit history (newest first). */
    suspend fun gitLog(path: String, limit: Int? = null): GitLogResult =
        decode(send("git_log", encode(GitLogBody(path, limit))), GitLogResult.serializer())

    /** Working-tree diff (bare unified-diff string; empty if none). */
    suspend fun gitDiff(path: String, file: String? = null): String =
        decodeBareString(send("git_diff", encode(GitDiffBody(path, file))))

    /** A single commit's diff (bare unified-diff string). Pass the full 40-char SHA. */
    suspend fun gitShowDiff(path: String, commit: String, file: String? = null): String =
        decodeBareString(send("git_show_diff", encode(GitShowDiffBody(path, commit, file))))

    /** Clone a repo into [targetDir] (full destination path). Long-running. */
    suspend fun cloneRepository(url: String, targetDir: String, credentials: GitCredentials?) {
        send("clone_repository", encode(CloneRepositoryBody(url, targetDir, credentials)))
    }

    /** Local + remote + worktree branch names. */
    suspend fun gitListAllBranches(path: String): GitBranchList =
        decode(send("git_list_all_branches", encode(PathBody(path))), GitBranchList.serializer())

    /** Current branch name, or null for detached HEAD / non-git. */
    suspend fun getGitBranch(path: String): String? {
        val text = send("get_git_branch", encode(PathBody(path)))
        if (isJsonNull(text)) return null
        return runCatching { CoaxiJson.response.decodeFromString(String.serializer(), text) }.getOrNull()
    }

    /** Check out an existing branch. */
    suspend fun gitCheckout(path: String, branchName: String) {
        send("git_checkout", encode(GitCheckoutBody(path, branchName)))
    }

    /** Create and check out a new branch. */
    suspend fun gitNewBranch(path: String, branchName: String, startPoint: String? = null) {
        send("git_new_branch", encode(GitNewBranchBody(path, branchName, startPoint)))
    }

    /** Resolve where [branch] is checked out, relative to the repo at [repoPath]. */
    suspend fun resolveWorktreeFolder(repoPath: String, branch: String): WorktreeResolution =
        decode(send("resolve_worktree_folder", encode(ResolveWorktreeFolderBody(repoPath, branch))), WorktreeResolution.serializer())

    /** Register a worktree directory as a folder (parented to [sourceFolderId]); returns it. */
    suspend fun openWorktreeFolder(path: String, sourceFolderId: Int): FolderDetail =
        decode(send("open_worktree_folder", encode(OpenWorktreeFolderBody(path, sourceFolderId))), FolderDetail.serializer())

    /**
     * Commit [files] (repo-root-relative paths) with [message]. The server stages
     * the listed files itself, so untracked paths commit directly. [folderId] lets
     * the server broadcast a commit event to other workspace clients.
     */
    suspend fun gitCommit(path: String, message: String, files: List<String>, folderId: Int? = null): GitCommitResult =
        decode(send("git_commit", encode(GitCommitBody(folderId, path, message, files))), GitCommitResult.serializer())

    /**
     * Push the current branch to [remote] (null = the branch's upstream / default).
     * [credentials] is optional — the server falls back to stored GitHub accounts and
     * throws `authentication_failed` when none match.
     */
    suspend fun gitPush(path: String, remote: String? = null, credentials: GitCredentials? = null, folderId: Int? = null): GitPushResult =
        decode(send("git_push", encode(GitPushBody(folderId, path, remote, credentials))), GitPushResult.serializer())

    /** Pull (fetch + merge) the current branch's upstream; reports changed files + any conflict. */
    suspend fun gitPull(path: String, credentials: GitCredentials? = null): GitPullResult =
        decode(send("git_pull", encode(GitPullBody(path, credentials))), GitPullResult.serializer())

    /** Fetch all remotes. Returns the raw fetch summary text (bare JSON string). */
    suspend fun gitFetch(path: String, credentials: GitCredentials? = null): String =
        decodeBareString(send("git_fetch", encode(GitFetchBody(path, credentials))))

    /** Branch + remotes + tracking remote — drives the Commits tab sync header. */
    suspend fun gitPushInfo(path: String): GitPushInfo =
        decode(send("git_push_info", encode(PathBody(path))), GitPushInfo.serializer())

    /** The repo's configured remotes (used to resolve the origin host for credentials). */
    suspend fun gitListRemotes(path: String): List<GitRemote> =
        decode(send("git_list_remotes", encode(PathBody(path))), ListSerializer(GitRemote.serializer()))

    /** Stage (`git add`) untracked/modified [files] so git starts tracking them. */
    suspend fun gitAddFiles(path: String, files: List<String>) {
        send("git_add_files", encode(GitAddFilesBody(path, files)))
    }

    /** Discard a tracked file's working-tree + staged changes (`git restore --source=HEAD`). */
    suspend fun gitRollbackFile(path: String, file: String) {
        send("git_rollback_file", encode(PathFileBody(path, file)))
    }

    /**
     * Delete a file (or directory) under [rootPath] from disk — [path] is relative
     * to [rootPath]. Used to remove an untracked file from the Changes tab (untracked
     * files have no HEAD to `gitRollbackFile` back to).
     */
    suspend fun deleteFileTreeEntry(rootPath: String, path: String) {
        send("delete_file_tree_entry", encode(DeleteFileTreeEntryBody(rootPath, path)))
    }

    /** Decode a bare JSON string response, tolerating empty/null as "". */
    private fun decodeBareString(text: String): String =
        if (isJsonNull(text)) "" else runCatching {
            CoaxiJson.response.decodeFromString(String.serializer(), text)
        }.getOrDefault("")

    /** Decode a camelCase-wire response (e.g. `list_directory_with_files`). */
    private fun <T> decodeCamel(text: String, deserializer: DeserializationStrategy<T>): T =
        runCatching { camelJson.decodeFromString(deserializer, text) }
            .getOrElse { throw ApiError.Decoding(it.message ?: "Failed to decode response") }

    // endregion

    // region Settings

    /** Reusable message templates. */
    suspend fun quickMessagesList(): List<QuickMessage> =
        decode(send("quick_messages_list", encode(EmptyBody)), ListSerializer(QuickMessage.serializer()))

    suspend fun quickMessageCreate(title: String, content: String): QuickMessage =
        decode(send("quick_messages_create", encode(QuickMessageCreateBody(title, content))), QuickMessage.serializer())

    suspend fun quickMessageUpdate(id: Int, title: String, content: String): QuickMessage =
        decode(send("quick_messages_update", encode(QuickMessageUpdateBody(id, title, content))), QuickMessage.serializer())

    suspend fun quickMessageDelete(id: Int) {
        send("quick_messages_delete", encode(IdBody(id)))
    }

    suspend fun quickMessagesReorder(ids: List<Int>) {
        send("quick_messages_reorder", encode(QuickMessageReorderBody(ids)))
    }

    /** "Live feedback" conversation-tool toggle. */
    suspend fun getFeedbackEnabled(): Boolean =
        decode(send("get_feedback_settings", encode(EmptyBody)), EnabledFlag.serializer()).enabled

    suspend fun setFeedbackEnabled(enabled: Boolean) {
        send("set_feedback_settings", encode(EnabledSettingsBody(EnabledFlag(enabled))))
    }

    /** "Ask user questions" conversation-tool toggle. */
    suspend fun getQuestionEnabled(): Boolean =
        decode(send("get_question_settings", encode(EmptyBody)), EnabledFlag.serializer()).enabled

    suspend fun setQuestionEnabled(enabled: Boolean) {
        send("set_question_settings", encode(EnabledSettingsBody(EnabledFlag(enabled))))
    }

    /** Multi-agent delegation settings (raw object; the UI patches scalars and re-sends). */
    suspend fun getDelegationSettings(): JsonObject =
        (runCatching { CoaxiJson.response.parseToJsonElement(send("get_delegation_settings", encode(EmptyBody))) }
            .getOrNull() as? JsonObject) ?: JsonObject(emptyMap())

    suspend fun setDelegationSettings(settings: JsonObject) {
        val body = JsonObject(mapOf("settings" to settings)).toString()
        send("set_delegation_settings", body)
    }

    /** Custom OpenAI-compatible model endpoints. */
    suspend fun listModelProviders(): List<ModelProviderInfo> =
        decode(send("list_model_providers", encode(EmptyBody)), ListSerializer(ModelProviderInfo.serializer()))

    suspend fun createModelProvider(name: String, apiUrl: String, apiKey: String, agentType: AgentType, model: String?) {
        send("create_model_provider", encode(CreateModelProviderBody(name, apiUrl, apiKey, agentType, model)))
    }

    suspend fun updateModelProvider(id: Int, name: String?, apiUrl: String?, apiKey: String?, agentType: AgentType?, model: String?) {
        send("update_model_provider", encode(UpdateModelProviderBody(id, name, apiUrl, apiKey, agentType, model)))
    }

    suspend fun deleteModelProvider(id: Int) {
        send("delete_model_provider", encode(IdBody(id)))
    }

    /** Server HTTP proxy settings. */
    suspend fun getSystemProxySettings(): SystemProxySettings =
        decode(send("get_system_proxy_settings", encode(EmptyBody)), SystemProxySettings.serializer())

    suspend fun updateSystemProxySettings(enabled: Boolean, proxyUrl: String?) {
        val body = buildJsonObject {
            putJsonObject("settings") {
                put("enabled", enabled)
                put("proxy_url", proxyUrl)
            }
        }.toString()
        send("update_system_proxy_settings", body)
    }

    /** Check for a server-side coaxi update (camelCase wire). */
    suspend fun checkAppUpdate(): AppUpdateCheckResult =
        decodeCamel(send("check_app_update", encode(EmptyBody)), AppUpdateCheckResult.serializer())

    /** Locally-configured MCP servers. */
    suspend fun mcpScanLocal(): List<LocalMcpServer> =
        decode(send("mcp_scan_local", encode(EmptyBody)), ListSerializer(LocalMcpServer.serializer()))

    suspend fun mcpUpsertLocalServer(serverId: String, spec: JsonElement, apps: List<String>) {
        val body = buildJsonObject {
            put("serverId", serverId)
            put("spec", spec)
            putJsonArray("apps") { apps.forEach { add(it) } }
        }.toString()
        send("mcp_upsert_local_server", body)
    }

    suspend fun mcpRemoveServer(serverId: String, apps: List<String>? = null) {
        val body = buildJsonObject {
            put("serverId", serverId)
            if (apps != null) putJsonArray("apps") { apps.forEach { add(it) } }
        }.toString()
        send("mcp_remove_server", body)
    }

    /**
     * Registered agents. Agent types this build doesn't recognise — a newer server
     * exposing more than the app knows (this build recognises every current server
     * type through [AgentType.CURSOR], so this is a forward-compat safety net) — are
     * DROPPED rather than left to collapse onto [AgentType.CLAUDE_CODE] during decode.
     * Otherwise each unknown agent would masquerade as Claude Code and the Agents
     * list, keyed by `agentType`, would hit duplicate keys and crash its LazyColumn.
     * See [decodeAgentList].
     */
    suspend fun acpListAgents(): List<AcpAgentInfo> {
        val text = send("acp_list_agents", encode(EmptyBody))
        return runCatching { decodeAgentList(text) }
            .getOrElse { throw ApiError.Decoding(it.message ?: "Failed to decode response") }
    }

    /**
     * Replace an agent's enabled flag, full env map, and model-provider link in one
     * call (`acp_update_agent_env`) — used both by the enable toggle and the
     * detail/config editor. Pass the *live* enabled value (toggling is a separate op).
     */
    suspend fun acpUpdateAgentEnv(agentType: AgentType, enabled: Boolean, env: Map<String, String>, modelProviderId: Int?) {
        send("acp_update_agent_env", encode(UpdateAgentEnvBody(agentType, enabled, env, modelProviderId)))
    }

    /**
     * Write an agent's native config files (`acp_update_agent_config`). Every payload
     * key is always sent (explicit `null` when not applicable for the agent type),
     * matching the web/iOS clients which never omit them.
     *
     * Grok's `XAI_API_KEY` rides the env (`acpUpdateAgentEnv`); its two controls + the
     * raw `~/.grok/config.toml` come through [grokConfigToml]. The Android client
     * patches the controls into the FULL config.toml itself (see AgentToml.patchGrok)
     * and sends that as [grokConfigToml] — deliberately leaving `grokStructured` an
     * explicit `null`. Sending `grokStructured` would omit the web-only custom-model /
     * `[session]` fields, and the server treats an omitted field as "delete", wiping
     * config the mobile panel never exposes; persisting our verbatim toml preserves it.
     *
     * Cursor's `cursorStructured` is safe to send, unlike Grok's: the server treats an
     * absent field there as "leave that key alone" (a patch), not "delete", so the
     * panel's two rule lists + sandbox mode merge onto the on-disk cli-config.json
     * while the CLI's own keys survive. A null field is therefore OMITTED, not sent as
     * an explicit null.
     */
    suspend fun acpUpdateAgentConfig(
        agentType: AgentType,
        configJson: String?,
        opencodeAuthJson: String?,
        codexAuthJson: String?,
        codexConfigToml: String?,
        grokConfigToml: String? = null,
        cursorCliConfigJson: String? = null,
        cursorStructured: CursorStructuredConfig? = null,
    ) {
        val body = buildJsonObject {
            put("agentType", agentType.wire)
            put("configJson", configJson)
            put("opencodeAuthJson", opencodeAuthJson)
            put("codexAuthJson", codexAuthJson)
            put("codexConfigToml", codexConfigToml)
            put("grokConfigToml", grokConfigToml)
            put("grokStructured", JsonNull)
            put("cursorCliConfigJson", cursorCliConfigJson)
            if (cursorStructured == null) {
                put("cursorStructured", JsonNull)
            } else {
                putJsonObject("cursorStructured") {
                    cursorStructured.sandboxMode?.let { put("sandboxMode", it) }
                    cursorStructured.permissionsAllow?.let { rules ->
                        putJsonArray("permissionsAllow") { rules.forEach { add(it) } }
                    }
                    cursorStructured.permissionsDeny?.let { rules ->
                        putJsonArray("permissionsDeny") { rules.forEach { add(it) } }
                    }
                }
            }
        }
        send("acp_update_agent_config", body.toString())
    }

    /**
     * Probe `cursor-agent status` for the Cursor panel's auth card. [apiKey] is the key
     * currently typed into the form (empty ⇒ test the browser login). Never throws for
     * an unauthenticated account — that's reported in the result — only for transport
     * failures.
     */
    suspend fun acpCursorAuthStatus(apiKey: String): CursorAuthStatus = decode(
        send("acp_cursor_auth_status", buildJsonObject { put("apiKey", apiKey) }.toString()),
        CursorAuthStatus.serializer(),
    )

    /**
     * Probe `cursor-agent models` for the Cursor panel's model picker. A probe that
     * could not run comes back as an empty list plus `error`.
     */
    suspend fun acpCursorListModels(apiKey: String): CursorModelsResult = decode(
        send("acp_cursor_list_models", buildJsonObject { put("apiKey", apiKey) }.toString()),
        CursorModelsResult.serializer(),
    )

    /**
     * Hermes' dedicated save path (`acp_update_hermes_config`). `apiKey`/`baseUrl`
     * follow [FieldEdit]: `Keep` omits the key, `Clear` sends explicit null
     * (= keep the stored secret), `Set` writes the value. `model`/`rawConfigYaml`
     * are omitted when null.
     */
    suspend fun acpUpdateHermesConfig(
        provider: String,
        model: String?,
        apiKey: FieldEdit,
        baseUrl: FieldEdit,
        rawConfigYaml: String? = null,
    ) {
        val body = buildJsonObject {
            put("provider", provider)
            if (model != null) put("model", model)
            when (apiKey) {
                FieldEdit.Keep -> {}
                FieldEdit.Clear -> put("apiKey", JsonNull)
                is FieldEdit.Set -> put("apiKey", apiKey.value)
            }
            when (baseUrl) {
                FieldEdit.Keep -> {}
                FieldEdit.Clear -> put("baseUrl", JsonNull)
                is FieldEdit.Set -> put("baseUrl", baseUrl.value)
            }
            if (rawConfigYaml != null) put("rawConfigYaml", rawConfigYaml)
        }
        send("acp_update_hermes_config", body.toString())
    }

    /** Kimi Code's dedicated save (apikey/login/raw). Returns affected running sessions. */
    suspend fun acpUpdateKimiCodeConfig(body: UpdateKimiCodeConfigBody): Int =
        decode(send("acp_update_kimi_code_config", encode(body)), Int.serializer())

    /** Probe a Kimi provider's `/models` endpoint (also validates the key). */
    suspend fun acpFetchKimiModels(baseUrl: String, apiKey: String): List<String> =
        decode(send("acp_fetch_kimi_models", encode(FetchKimiModelsBody(baseUrl, apiKey))), ListSerializer(String.serializer()))

    /**
     * Read pi's native config projection (defaults + linked/custom providers). The
     * response is **camelCase** (unlike most responses), so it's decoded accordingly.
     */
    suspend fun acpLoadPiConfig(): PiConfigProjection =
        decodeCamel(send("acp_load_pi_config", encode(EmptyBody)), PiConfigProjection.serializer())

    /** pi's credentials/model save (native settings.json / auth.json / models.json). */
    suspend fun acpUpdatePiConfig(body: UpdatePiConfigBody) {
        send("acp_update_pi_config", encode(body))
    }

    /** Validate a BYO-pi command/binary path. Not-found is a normal (non-throwing) result. */
    suspend fun acpValidatePiCommand(command: String): PiCommandValidation =
        decodeCamel(send("acp_validate_pi_command", encode(ValidatePiCommandBody(command))), PiCommandValidation.serializer())

    /** Install / uninstall the global `pi` binary that pi-acp spawns (long-running). */
    suspend fun acpInstallPiBinary(taskId: String) {
        send("acp_install_pi_binary", encode(TaskIdBody(taskId)))
    }

    suspend fun acpUninstallPiBinary(taskId: String) {
        send("acp_uninstall_pi_binary", encode(TaskIdBody(taskId)))
    }

    /** Git installation detection / custom path. */
    suspend fun detectGit(): GitDetectResult =
        decode(send("detect_git", encode(EmptyBody)), GitDetectResult.serializer())

    suspend fun testGitPath(path: String): GitDetectResult =
        decode(send("test_git_path", encode(PathBody(path))), GitDetectResult.serializer())

    suspend fun getGitSettings(): GitSettings =
        decode(send("get_git_settings", encode(EmptyBody)), GitSettings.serializer())

    suspend fun updateGitSettings(customPath: String?) {
        val body = buildJsonObject { putJsonObject("settings") { put("custom_path", customPath) } }.toString()
        send("update_git_settings", body)
    }

    /** Linked GitHub accounts. */
    suspend fun getGitHubAccounts(): List<GitHubAccount> =
        decode(send("get_github_accounts", encode(EmptyBody)), GitHubAccountList.serializer()).accounts

    /** Validate a GitHub token against [serverUrl] before saving it as an account. */
    suspend fun validateGithubToken(serverUrl: String, token: String): GitHubTokenValidation =
        decode(send("validate_github_token", encode(ValidateGitHubTokenBody(serverUrl, token))), GitHubTokenValidation.serializer())

    /** Persist an account's keyring token (`save_account_token`). */
    suspend fun saveAccountToken(accountId: String, token: String) {
        send("save_account_token", encode(AccountTokenBody(accountId, token)))
    }

    /**
     * Full-replace the linked GitHub accounts. Wrapped `{settings:{accounts:[…]}}`
     * with **snake_case** account keys (this endpoint mirrors the server's settings
     * store, unlike the usual camelCase request bodies — same shape as
     * `update_git_settings`).
     */
    suspend fun updateGithubAccounts(accounts: List<GitHubAccount>) {
        val body = buildJsonObject {
            putJsonObject("settings") {
                putJsonArray("accounts") {
                    accounts.forEach { acc ->
                        add(buildJsonObject {
                            put("id", acc.id)
                            put("server_url", acc.serverUrl)
                            put("username", acc.username)
                            put("is_default", acc.isDefault)
                            putJsonArray("scopes") { acc.scopes.forEach { add(it) } }
                            acc.avatarUrl?.let { put("avatar_url", it) }
                            acc.createdAt?.let { put("created_at", it) }
                        })
                    }
                }
            }
        }.toString()
        send("update_github_accounts", body)
    }

    /** Per-agent markdown skills. */
    suspend fun listAgentSkills(agentType: AgentType): AgentSkillsListResult =
        decode(send("acp_list_agent_skills", encode(SkillListBody(agentType))), AgentSkillsListResult.serializer())

    suspend fun readAgentSkill(agentType: AgentType, scope: String, skillId: String): AgentSkillContent =
        decode(send("acp_read_agent_skill", encode(SkillReadBody(agentType, scope, skillId))), AgentSkillContent.serializer())

    suspend fun saveAgentSkill(agentType: AgentType, scope: String, skillId: String, content: String, layout: String?) {
        send("acp_save_agent_skill", encode(SkillSaveBody(agentType, scope, skillId, content, layout = layout)))
    }

    suspend fun deleteAgentSkill(agentType: AgentType, scope: String, skillId: String) {
        send("acp_delete_agent_skill", encode(SkillDeleteBody(agentType, scope, skillId)))
    }

    /** The global built-in expert catalog (`experts_list`). */
    suspend fun expertsList(): List<ExpertListItem> =
        decode(send("experts_list", encode(EmptyBody)), ListSerializer(ExpertListItem.serializer()))

    /** Experts linked to one agent — used by the compose "+" Expert Skills menu (M10). */
    suspend fun expertsListForAgent(agentType: AgentType): List<ExpertListItem> =
        decode(send("experts_list_for_agent", encode(ExpertAgentBody(agentType))), ListSerializer(ExpertListItem.serializer()))

    /** An expert's markdown content (`experts_read_content`, bare string). */
    suspend fun expertReadContent(expertId: String): String =
        decode(send("experts_read_content", encode(ExpertIdBody(expertId))), String.serializer())

    /** Per-agent link state of an expert (camelCase wire). */
    suspend fun expertInstallStatus(expertId: String): List<ExpertInstallStatus> =
        decodeCamel(send("experts_get_install_status", encode(ExpertIdBody(expertId))), ListSerializer(ExpertInstallStatus.serializer()))

    /** Link an expert into an agent's skills; returns the resulting status. */
    suspend fun expertLink(expertId: String, agentType: AgentType): ExpertInstallStatus =
        decodeCamel(send("experts_link_to_agent", encode(ExpertLinkBody(expertId, agentType))), ExpertInstallStatus.serializer())

    suspend fun expertUnlink(expertId: String, agentType: AgentType) {
        send("experts_unlink_from_agent", encode(ExpertLinkBody(expertId, agentType)))
    }

    // region Chat channels

    suspend fun listChatChannels(): List<ChatChannelInfo> =
        decode(send("list_chat_channels", encode(EmptyBody)), ListSerializer(ChatChannelInfo.serializer()))

    suspend fun chatChannelStatus(): List<ChannelStatusInfo> =
        decode(send("get_chat_channel_status", encode(EmptyBody)), ListSerializer(ChannelStatusInfo.serializer()))

    suspend fun createChatChannel(
        name: String, channelType: ChannelType, configJson: String,
        enabled: Boolean, dailyReportEnabled: Boolean, dailyReportTime: String?,
    ): ChatChannelInfo = decode(
        send("create_chat_channel", encode(CreateChatChannelBody(name, channelType, configJson, enabled, dailyReportEnabled, dailyReportTime))),
        ChatChannelInfo.serializer(),
    )

    suspend fun updateChatChannel(body: UpdateChatChannelBody): ChatChannelInfo {
        val json = buildJsonObject {
            put("id", body.id)
            body.name?.let { put("name", it) }
            body.enabled?.let { put("enabled", it) }
            body.configJson?.let { put("configJson", it) }
            body.dailyReportEnabled?.let { put("dailyReportEnabled", it) }
            when (val t = body.dailyReportTime) {
                FieldEdit.Keep -> {}
                FieldEdit.Clear -> put("dailyReportTime", JsonNull)
                is FieldEdit.Set -> put("dailyReportTime", t.value)
            }
        }
        return decode(send("update_chat_channel", json.toString()), ChatChannelInfo.serializer())
    }

    suspend fun deleteChatChannel(id: Int) { send("delete_chat_channel", encode(IdBody(id))) }
    suspend fun connectChatChannel(id: Int) { send("connect_chat_channel", encode(IdBody(id))) }
    suspend fun disconnectChatChannel(id: Int) { send("disconnect_chat_channel", encode(IdBody(id))) }
    suspend fun testChatChannel(id: Int) { send("test_chat_channel", encode(IdBody(id))) }

    suspend fun saveChatChannelToken(channelId: Int, token: String) {
        send("save_chat_channel_token", encode(ChannelTokenBody(channelId, token)))
    }

    suspend fun chatChannelHasToken(channelId: Int): Boolean =
        runCatching { decode(send("get_chat_channel_has_token", encode(ChannelIdOnlyBody(channelId))), Boolean.serializer()) }.getOrDefault(false)

    suspend fun deleteChatChannelToken(channelId: Int) {
        send("delete_chat_channel_token", encode(ChannelIdOnlyBody(channelId)))
    }

    suspend fun listChatChannelMessages(channelId: Int, limit: Int? = 50, offset: Int? = null): List<ChatChannelMessageLog> =
        decode(send("list_chat_channel_messages", encode(ListChannelMessagesBody(channelId, limit, offset))), ListSerializer(ChatChannelMessageLog.serializer()))

    suspend fun weixinGetQrcode(): WeixinQrcode =
        decode(send("weixin_get_qrcode", encode(EmptyBody)), WeixinQrcode.serializer())

    /** Returns the raw status string (e.g. "pending"/"scanned"/"confirmed"/"expired"). */
    suspend fun weixinCheckQrcode(channelId: Int, qrcode: String): String =
        decode(send("weixin_check_qrcode", encode(WeixinCheckBody(channelId, qrcode))), WeixinQrStatus.serializer()).status

    // Global chat behavior settings. GET endpoints return bare JSON fragments.

    suspend fun chatCommandPrefix(): String =
        runCatching { decode(send("get_chat_command_prefix", encode(EmptyBody)), String.serializer()) }.getOrDefault("/")

    suspend fun setChatCommandPrefix(prefix: String) { send("set_chat_command_prefix", encode(ChatPrefixBody(prefix))) }

    suspend fun chatMessageLanguage(): String =
        runCatching { decode(send("get_chat_message_language", encode(EmptyBody)), String.serializer()) }.getOrDefault("en")

    suspend fun setChatMessageLanguage(language: String) { send("set_chat_message_language", encode(ChatLanguageBody(language))) }

    /** `null` = the server's default-on set (it stored `null`). */
    suspend fun chatEventFilter(): List<String>? =
        runCatching { decode(send("get_chat_event_filter", encode(EmptyBody)), ListSerializer(String.serializer())) }.getOrNull()

    suspend fun setChatEventFilter(filter: List<String>?) {
        val json = buildJsonObject {
            if (filter == null) put("filter", JsonNull)
            else putJsonArray("filter") { filter.forEach { add(it) } }
        }
        send("set_chat_event_filter", json.toString())
    }

    suspend fun chatEventWebhooks(): List<WebhookConfig> =
        decode(send("get_chat_event_webhooks", encode(EmptyBody)), ListSerializer(WebhookConfig.serializer()))

    suspend fun setChatEventWebhooks(webhooks: List<WebhookConfig>) {
        send("set_chat_event_webhooks", encode(ChatWebhooksBody(webhooks)))
    }

    // endregion

    // region Session snapshot / agent options

    /** Authoritative live snapshot by conversation; null when no live session exists. */
    suspend fun sessionSnapshotByConversation(conversationId: Int): SessionSnapshot? {
        val text = send("acp_get_session_snapshot_by_conversation", encode(ConversationIdBody(conversationId)))
        if (isJsonNull(text)) return null
        return decode(text, SessionSnapshot.serializer())
    }

    /** Authoritative live snapshot by connection id (for reconcile after apply). */
    suspend fun sessionSnapshot(connectionId: String): SessionSnapshot? {
        val text = send("acp_get_session_snapshot", encode(ConnectionIdBody(connectionId)))
        if (isJsonNull(text)) return null
        return decode(text, SessionSnapshot.serializer())
    }

    /** Per-agent catalog of modes + config (probes the agent; slow). */
    suspend fun describeAgentOptions(agentType: AgentType, workingDir: String?): AgentOptionsSnapshot =
        decode(send("acp_describe_agent_options", encode(DescribeAgentOptionsBody(agentType, workingDir))), AgentOptionsSnapshot.serializer())

    suspend fun acpSetMode(connectionId: String, modeId: String) {
        send("acp_set_mode", encode(SetModeBody(connectionId, modeId)))
    }

    suspend fun acpSetConfigOption(connectionId: String, configId: String, valueId: String) {
        send("acp_set_config_option", encode(SetConfigOptionBody(connectionId, configId, valueId)))
    }

    // endregion

    // endregion

    // region Transport

    /**
     * `POST /api/<path>` with [bodyJson] as the body. Returns the raw response
     * text, or throws an [ApiError] for any non-2xx / transport failure.
     */
    private suspend fun send(path: String, bodyJson: String): String {
        val url = if(path=="health")"$baseUrl/api/v1/probe" else "$baseUrl/api/$path"
        val response = try {
            http.post(url) {
                header(HttpHeaders.Authorization, "Bearer $token")
                contentType(ContentType.Application.Json)
                setBody(if(path=="health")buildJsonObject{put("protocolVersion",4);put("administratorToken",token)}.toString()else bodyJson)
            }
        } catch (e: kotlinx.coroutines.CancellationException) {
            // Cancellation must stay cancellation — never masquerade as a transport error,
            // or a caller (e.g. the send path) would treat a user Stop as a real send failure.
            throw e
        } catch (e: Exception) {
            throw ApiError.Transport(e.message ?: "Network error")
        }

        val text = response.bodyAsText()
        val code = response.status.value
        if (code !in 200..299) {
            if (code == 401) throw ApiError.Unauthorized
            val parsed = runCatching {
                CoaxiJson.response.decodeFromString(ServerError.serializer(), text)
            }.getOrNull()
            if (code == 409 || parsed?.code == "turn_in_progress") throw ApiError.TurnInProgress
            throw ApiError.Server(
                status = code,
                code = parsed?.code,
                serverMessage = parsed?.message?.takeIf { it.isNotBlank() }
                    ?: response.status.description,
            )
        }
        return text
    }

    private inline fun <reified T> encode(body: T): String =
        CoaxiJson.request.encodeToString(body)

    private fun <T> decode(text: String, deserializer: DeserializationStrategy<T>): T =
        runCatching { CoaxiJson.response.decodeFromString(deserializer, text) }
            .getOrElse { throw ApiError.Decoding(it.message ?: "Failed to decode response") }

    // endregion

    companion object {
        /** Trim a trailing slash so `"$baseUrl/api/<name>"` never doubles up. */
        fun normalizeBaseUrl(raw: String): String =
            raw.trim().trimEnd('/')

        /**
         * `acp_connect` returns a bare JSON string connection id. Require exactly
         * that: a non-empty JSON string. A `null`, object, array, or empty body
         * is rejected so we never attach/prompt against a bogus id.
         */
        fun decodeConnectionId(text: String): String {
            val raw = runCatching {
                CoaxiJson.response.decodeFromString(String.serializer(), text)
            }.getOrElse { throw ApiError.Decoding("acp_connect did not return a connection id string") }
            val id = raw.trim()
            if (id.isEmpty()) throw ApiError.Decoding("acp_connect returned an empty connection id")
            return id
        }

        /**
         * Decode an `acp_list_agents` response, dropping any entry whose
         * `agent_type` is unknown to this build (it would otherwise decode to
         * [AgentType.CLAUDE_CODE] and collide with the real Claude Code row) and
         * de-duplicating by agent type — the Agents/Experts/Skills lists are keyed
         * by `agentType`, so a repeated type would crash their LazyColumn.
         */
        fun decodeAgentList(text: String): List<AcpAgentInfo> {
            val serializer = ListSerializer(AcpAgentInfo.serializer())
            val decoded = when (val root = CoaxiJson.response.parseToJsonElement(text)) {
                is JsonArray -> {
                    val known = root.filter { el ->
                        val wire = ((el as? JsonObject)?.get("agent_type") as? JsonPrimitive)
                            ?.takeIf { it.isString }?.content
                        wire != null && AgentType.knownFromWire(wire) != null
                    }
                    CoaxiJson.response.decodeFromJsonElement(serializer, JsonArray(known))
                }
                else -> CoaxiJson.response.decodeFromString(serializer, text)
            }
            return decoded.distinctBy { it.agentType }
        }

        /** True when the raw response is empty or the JSON literal `null`. */
        fun isJsonNull(text: String): Boolean {
            val s = text.trim()
            return s.isEmpty() || s == "null"
        }

        /** Codec for camelCase-wire responses (e.g. `list_directory_with_files`). */
        private val camelJson = Json {
            ignoreUnknownKeys = true
            isLenient = true
            coerceInputValues = true
        }
    }
}
