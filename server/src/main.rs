
use std::collections::HashMap;

use nano_crl2::core::syntax::SourceCursorPos;

use nano_crl2_lsp::core::{Editor, EditorConfig};
use nano_crl2_lsp::lsp_context::LspContext;
use nano_crl2_lsp::semantic_token::{
    get_semantic_tokens_from_tokens,
    SEMANTIC_TOKEN_MAP,
};
use nano_crl2_lsp::util::source_range_to_lsp_range;

use serde_json::Value;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{*, notification::ShowMessage};
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        lsp_context: LspContext::new(),
        editor_config: EditorConfig {
            editor: Editor::VsCode,
            check_parse_errors_continuously: true,
            check_errors_continuously: false,
        },
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}

struct Backend {
    client: Client,
    lsp_context: LspContext,
    editor_config: EditorConfig,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            offset_encoding: None,
            capabilities: ServerCapabilities {
                inlay_hint_provider: None,
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("mcrl2".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: SEMANTIC_TOKEN_MAP.into(),
                                    token_modifiers: vec![],
                                },
                                range: Some(true),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
        })
        .await
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: std::mem::take(&mut params.content_changes[0].text),
            version: params.text_document.version,
        }).await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let document_uri = params.text_document_position_params.text_document.uri.clone();
        let position = params.text_document_position_params.position;

        self.client.log_message(
            MessageType::LOG,
            &format!("goto_definition {:?} {:?}", document_uri.path(), position),
        ).await;

        match self.cursor_source_click(document_uri, position).await? {
            Some(CursorSourceClick::Definition(value)) => {
                Ok(Some(GotoDefinitionResponse::Link(vec![value])))
            },
            Some(CursorSourceClick::References(..)) => Ok(None),
            None => Ok(None),
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let document_uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        self.client.log_message(
            MessageType::LOG,
            &format!("references {:?} {:?}", document_uri.path(), position),
        ).await;

        match self.cursor_source_click(document_uri, position).await? {
            Some(CursorSourceClick::Definition(..)) => Ok(None),
            Some(CursorSourceClick::References(values, _)) => {
                Ok(Some(values))
            },
            None => Ok(None),
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        self.client
            .log_message(MessageType::LOG, "semantic_tokens_full")
            .await;

        let document_uri = params.text_document.uri.to_string();
        let Ok(tokens) = self.lsp_context.query_token_list(&document_uri) else {
            // TODO report error
            return Ok(None);
        };

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: get_semantic_tokens_from_tokens(&tokens, &self.editor_config),
        })))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        self.client
            .log_message(MessageType::LOG, "semantic_tokens_range")
            .await;

        let document_uri = params.text_document.uri.to_string();

        // Ok(Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
        //     result_id: None,
        //     data: semantic_tokens,
        // })))
        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let document_uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // let offset = char + position.character as usize;

        // TODO

        // let completions = completion(&ast, offset);
        // let mut result = Vec::with_capacity(completions.len());
        // for (_, item) in completions {
        //     match item {
        //         nano_crl2_lsp::completion::ImCompleteCompletionItem::Variable(var) => {
        //             result.push(CompletionItem {
        //                 label: var.clone(),
        //                 insert_text: Some(var.clone()),
        //                 kind: Some(CompletionItemKind::VARIABLE),
        //                 detail: Some(var),
        //                 ..Default::default()
        //             });
        //         }
        //         nano_crl2_lsp::completion::ImCompleteCompletionItem::Function(
        //             name,
        //             args,
        //         ) => {
        //             result.push(CompletionItem {
        //                 label: name.clone(),
        //                 kind: Some(CompletionItemKind::FUNCTION),
        //                 detail: Some(name.clone()),
        //                 insert_text: Some(format!(
        //                     "{}({})",
        //                     name,
        //                     args.iter()
        //                         .enumerate()
        //                         .map(|(index, item)| { format!("${{{}:{}}}", index + 1, item) })
        //                         .collect::<Vec<_>>()
        //                         .join(",")
        //                 )),
        //                 insert_text_format: Some(InsertTextFormat::SNIPPET),
        //                 ..Default::default()
        //             });
        //         }
        //     }
        // }

        // Ok(Some(CompletionResponse::Array(result)))
        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let document_uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let message = format!(
            "rename {:?} {:?} {:?}",
            document_uri.path(),
            position,
            new_name,
        );
        self.client.log_message(MessageType::LOG, &message).await;

        match self.cursor_source_click(document_uri.clone(), position).await? {
            Some(CursorSourceClick::Definition(_value)) => {
                // TODO: call cursor_source_click again with value.target_range
                Ok(None)
            },
            Some(CursorSourceClick::References(values, def_range)) => {
                let mut edits = HashMap::new();
                for value in values {
                    edits.entry(value.uri)
                        .or_insert(Vec::new())
                        .push(TextEdit::new(value.range, new_name.clone()));
                }
                edits.entry(document_uri)
                    .or_insert(Vec::new())
                    .push(TextEdit::new(def_range, new_name.clone()));
                Ok(Some(WorkspaceEdit::new(edits)))
            },
            None => Ok(None),
        }
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "configuration changed!")
            .await;
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        self.client
            .log_message(MessageType::INFO, "workspace folders changed!")
            .await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.client
            .log_message(MessageType::INFO, "watched files have changed!")
            .await;
    }

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        self.client
            .log_message(MessageType::INFO, "command executed!")
            .await;

        // match self.client.apply_edit(WorkspaceEdit::default()).await {
        //     Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
        //     Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
        //     Err(err) => self.client.log_message(MessageType::ERROR, err).await,
        // }

        Ok(None)
    }
}

struct TextDocumentItem {
    uri: Url,
    text: String,
    version: i32,
}

impl Backend {
    async fn on_change(&self, params: TextDocumentItem) {
        match self.lsp_context.set_file(params.uri.to_string(), params.text.clone()) {
            Ok(()) => {},
            Err(()) => {
                let msg = "could not update contents of text file in nano-crl2-lsp, for unknown reasons";
                self.client.send_notification::<ShowMessage>(ShowMessageParams {
                    typ: MessageType::ERROR,
                    message: msg.to_owned(),
                }).await;
                return;
            },
        };

        let diagnostics = if self.editor_config.check_errors_continuously {
            todo!();
        } else if self.editor_config.check_parse_errors_continuously {
            if self.lsp_context.query_ast(params.uri.as_str()).is_err() {
                self.lsp_context.get_diagnostics()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        self.client.publish_diagnostics(
            params.uri,
            diagnostics,
            Some(params.version),
        ).await;
    }

    async fn cursor_source_click(
        &self,
        document_uri: Url,
        position: Position,
    ) -> Result<Option<CursorSourceClick>> {
        let (source_loc, node_id, is_def) = match self.lsp_context.query_identifier_node_at_loc(
            &document_uri.to_string(),
            SourceCursorPos::new(position.line, position.character),
        ) {
            Ok(Some(value)) => value,
            Ok(None) => {
                self.client.log_message(
                    MessageType::INFO,
                    &"the cursor is not pointing at a node",
                ).await;
                return Ok(None);
            },
            Err(()) => {
                self.client.log_message(
                    MessageType::INFO,
                    &"error found while trying to find clicked node",
                ).await;
                return Ok(None);
            },
        };

        if is_def {
            self.client.log_message(
                MessageType::INFO,
                &format!("finding references for {:?}", node_id),
            ).await;

            let references = match self.lsp_context.query_references(node_id) {
                Ok(value) => value,
                Err(()) => {
                    self.client.log_message(
                        MessageType::INFO,
                        &"could not find references (reliably)",
                    ).await;
                    return Ok(None)
                },
            };

            let result = references.into_iter()
                .map(|reference| Location::new(
                    document_uri.clone(),
                    source_range_to_lsp_range(reference)
                ))
                .collect::<Vec<_>>();

            let def_range = source_range_to_lsp_range(source_loc);
            Ok(Some(CursorSourceClick::References(result, def_range)))
        } else {
            self.client.log_message(MessageType::INFO, &format!(
                "trying to go to definition of {:?} at {:?}...",
                node_id,
                source_loc,
            )).await;

            let (identifier_loc, symbol_loc) = match self.lsp_context.query_definition(node_id) {
                Ok(value) => value,
                Err(()) => return Ok(None),
            };

            self.client.log_message(MessageType::INFO, &format!(
                "found location {:?} inside {:?}!",
                identifier_loc,
                symbol_loc,
            )).await;

            Ok(Some(CursorSourceClick::Definition(LocationLink {
                origin_selection_range: Some(source_range_to_lsp_range(source_loc)),
                // NOTE: this is temporary, might not be correct
                target_uri: document_uri,
                target_range: source_range_to_lsp_range(symbol_loc),
                target_selection_range: source_range_to_lsp_range(identifier_loc),
            })))
        }
    }
}

enum CursorSourceClick {
    Definition(LocationLink),
    References(Vec<Location>, Range),
}
