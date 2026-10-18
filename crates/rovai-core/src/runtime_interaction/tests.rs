use super::*;

fn request(protocol: Protocol, question: bool, params: Value) -> Request {
    let (adapter, method) = match (protocol, question) {
        (Protocol::CursorAcp, true) => (AdapterKind::CursorAgent, "cursor/ask_question"),
        (Protocol::CursorAcp, false) => (AdapterKind::CursorAgent, "cursor/create_plan"),
        (Protocol::GrokAcp, true) => (AdapterKind::GrokBuild, "_x.ai/ask_user_question"),
        (Protocol::GrokAcp, false) => (AdapterKind::GrokBuild, "_x.ai/exit_plan_mode"),
    };
    Request::from_acp(adapter, method, &params)
        .unwrap()
        .unwrap()
}
fn answer(labels: &[&[&str]]) -> Answer {
    Answer::Question {
        declined: false,
        answers: labels
            .iter()
            .enumerate()
            .map(|(i, labels)| QuestionAnswer {
                question_id: format!("q{i}"),
                labels: labels.iter().map(|s| s.to_string()).collect(),
            })
            .collect(),
    }
}

// Lowest-cost owner for the native payload boundary. Delegation contract tests
// own model tool inputs; Fleet/permission tests do not encode these native replies.
#[test]
fn native_questions_and_plan_decisions_preserve_wire_identity_and_user_words() {
    let params = json!({"toolCallId":"native-call","title":"  配置选择与后续工程规划  ","questions":[
        {"id":" native-1 ","prompt":"选择模式？","options":[{"id":" choice-1 ","label":"审查"},{"id":"choice-2","label":"实现"}]},
        {"id":"native-2","prompt":"选择模块？","allowMultiple":true,"options":[{"id":"api","label":"接口"},{"id":"ui","label":"界面"}]}
    ]});
    let cursor = request(Protocol::CursorAcp, true, params.clone());
    // Determinism is the durable original-request reconciliation property.
    assert_eq!(cursor, request(Protocol::CursorAcp, true, params));
    assert_eq!(cursor.kind(), "question");
    assert_eq!(
        cursor
            .reply(&answer(&[&["审查"], &["界面", "接口"]]))
            .unwrap()
            .native_response,
        json!({"outcome":{"outcome":"answered","answers":[
            {"questionId":" native-1 ","selectedOptionIds":[" choice-1 "]},
            {"questionId":"native-2","selectedOptionIds":["ui","api"]}
        ]}})
    );
    let mixed = cursor
        .reply(&answer(&[&["审查"], &["接口", "需要先兼容旧版"]]))
        .unwrap();
    assert_eq!(mixed.native_response["outcome"]["outcome"], "skipped");
    let reason = mixed.native_response["outcome"]["reason"].as_str().unwrap();
    assert!(
        reason.contains("审查") && reason.contains("接口") && reason.contains("需要先兼容旧版")
    );
    assert!(mixed.native_response["outcome"].get("answers").is_none());
    let declined = Answer::Question {
        answers: vec![],
        declined: true,
    };
    assert_eq!(
        cursor.reply(&declined).unwrap().native_response,
        cursor_outcome("skipped")
    );

    let grok = request(
        Protocol::GrokAcp,
        true,
        json!({"questions":[
            {"question":"选择模式？","options":[{"label":"审查","description":"先找风险"},{"label":"实现"}]},
            {"question":"选择模块？","multiSelect":true,"options":[{"label":"接口"},{"label":"界面"}]}
        ]}),
    );
    assert_eq!(
        grok.reply(&answer(&[&["审查"], &["接口", "其他模块"]]))
            .unwrap()
            .native_response,
        json!({"outcome":"accepted","answers":{"选择模式？":"审查","选择模块？":["接口","其他模块"]},"partial_answers":{}})
    );
    assert_eq!(
        grok.reply(&declined).unwrap().native_response,
        json!({"outcome":"skip_interview"})
    );

    for protocol in [Protocol::CursorAcp, Protocol::GrokAcp] {
        let plan = request(
            protocol,
            false,
            if protocol == Protocol::CursorAcp {
                json!({"plan":"","overview":"原版方案","toolCallId":"plan-call"})
            } else {
                json!({"planContent":"原版方案","toolCallId":"plan-call"})
            },
        );
        assert_eq!(plan.kind(), "plan_approval");
        assert_eq!(
            plan.body,
            RequestBody::PlanApproval {
                plan_markdown: "原版方案".into()
            }
        );
        for (decision, feedback, cursor_expected, grok_expected, followup) in [
            (
                PlanDecision::Approve,
                None,
                cursor_outcome("accepted"),
                json!({"outcome":"approved","feedback":""}),
                None,
            ),
            (
                PlanDecision::RequestChanges,
                Some("请保留原目录".to_string()),
                json!({"outcome":{"outcome":"rejected","reason":"请保留原目录"}}),
                json!({"outcome":"keep_planning","feedback":"请保留原目录"}),
                Some("请保留原目录".to_string()),
            ),
            (
                PlanDecision::Abandon,
                None,
                cursor_outcome("cancelled"),
                json!({"outcome":"abandoned","feedback":""}),
                None,
            ),
        ] {
            let reply = plan
                .reply(&Answer::PlanApproval { decision, feedback })
                .unwrap();
            assert_eq!(
                reply.native_response,
                if protocol == Protocol::CursorAcp {
                    cursor_expected
                } else {
                    grok_expected
                }
            );
            assert_eq!(
                reply.followup_feedback,
                if protocol == Protocol::GrokAcp {
                    followup
                } else {
                    None
                }
            );
        }
        let empty = request(protocol, false, json!({}));
        assert_eq!(
            empty.body,
            RequestBody::PlanApproval {
                plan_markdown: String::new()
            }
        );
        let interrupted = plan.interrupted_response();
        assert_eq!(
            interrupted,
            if protocol == Protocol::CursorAcp {
                cursor_outcome("cancelled")
            } else {
                json!({"outcome":"keep_planning","feedback":""})
            }
        );
        for response in [
            Answer::PlanApproval {
                decision: PlanDecision::RequestChanges,
                feedback: None,
            },
            Answer::PlanApproval {
                decision: PlanDecision::Abandon,
                feedback: Some("不能丢弃".into()),
            },
            Answer::PlanApproval {
                decision: PlanDecision::RequestChanges,
                feedback: Some("中".repeat(MAX_FEEDBACK_CHARS + 1)),
            },
            declined.clone(),
        ] {
            assert!(plan.reply(&response).is_err());
        }
    }
    let with_feedback = request(Protocol::GrokAcp, false, json!({"planContent":null}));
    assert_eq!(
        with_feedback
            .reply(&Answer::PlanApproval {
                decision: PlanDecision::Approve,
                feedback: Some("  保留证据  ".into())
            })
            .unwrap()
            .native_response,
        json!({"outcome":"approved","feedback":"保留证据"})
    );

    let base = json!({"questions":[{"id":"id","prompt":"选择？","options":[{"id":"a","label":"甲"},{"id":"b","label":"乙"}]}]});
    for (field, value) in [
        ("questions", json!([])),
        ("questions", json!(vec![base["questions"][0].clone(); 5])),
        ("questions", json!(null)),
    ] {
        let mut bad = base.clone();
        bad[field] = value;
        assert!(Request::from_acp(AdapterKind::CursorAgent, "cursor/ask_question", &bad).is_err());
    }
    for (field, value) in [
        ("options", json!([])),
        (
            "options",
            json!([{"id":"a","label":"甲"},{"id":"b","label":" 甲 "}]),
        ),
        (
            "options",
            json!([{"id":"same","label":"甲"},{"id":"same","label":"乙"}]),
        ),
        ("prompt", json!("中".repeat(MAX_TEXT_CHARS + 1))),
        ("allowMultiple", json!("true")),
    ] {
        let mut bad = base.clone();
        bad["questions"][0][field] = value;
        assert!(Request::from_acp(AdapterKind::CursorAgent, "cursor/ask_question", &bad).is_err());
    }
    let mut duplicate = base.clone();
    duplicate["questions"]
        .as_array_mut()
        .unwrap()
        .push(base["questions"][0].clone());
    assert!(
        Request::from_acp(AdapterKind::CursorAgent, "cursor/ask_question", &duplicate).is_err()
    );
    let mut duplicate_grok = serde_json::to_value(&grok).unwrap();
    duplicate_grok["body"]["questions"][1]["question"] = json!("选择模式？");
    assert!(
        serde_json::from_value::<Request>(duplicate_grok)
            .unwrap()
            .validate()
            .is_err()
    );
    assert!(
        Request::from_acp(
            AdapterKind::GrokBuild,
            "_x.ai/exit_plan_mode",
            &json!({"planContent":"中".repeat(MAX_PLAN_CHARS+1)})
        )
        .is_err()
    );
    assert!(
        Request::from_acp(
            AdapterKind::CursorAgent,
            "cursor/create_plan",
            &json!({"plan":{}})
        )
        .is_err()
    );
    for (adapter, method) in [
        (AdapterKind::CodexCli, "cursor/ask_question"),
        (AdapterKind::CursorAgent, "_x.ai/ask_user_question"),
        (AdapterKind::GrokBuild, "cursor/create_plan"),
    ] {
        assert!(Request::from_acp(adapter, method, &base).unwrap().is_none());
    }
    for bad in [
        answer(&[&["甲", "乙"]]),
        answer(&[&[""]]),
        answer(&[&["中".repeat(MAX_TEXT_CHARS + 1).as_str()]]),
        Answer::Question {
            answers: vec![QuestionAnswer {
                question_id: "foreign".into(),
                labels: vec!["甲".into()],
            }],
            declined: false,
        },
        Answer::Question {
            answers: vec![QuestionAnswer {
                question_id: "q0".into(),
                labels: vec!["甲".into()],
            }],
            declined: true,
        },
    ] {
        assert!(
            request(Protocol::CursorAcp, true, base.clone())
                .reply(&bad)
                .is_err()
        );
    }
    let encoded = serde_json::to_value(&cursor).unwrap();
    assert_eq!(
        serde_json::from_value::<Request>(encoded.clone()).unwrap(),
        cursor
    );
    let mut unknown = encoded;
    unknown["actor"] = json!("user");
    assert!(serde_json::from_value::<Request>(unknown).is_err());
    assert!(serde_json::from_value::<Answer>(json!({"kind":"plan_approval","decision":"approve","feedback":null,"agentRunId":"foreign"})).is_err());
    let mut future = cursor.clone();
    future.schema_version = 2;
    assert!(future.reply(&declined).is_err());
}
