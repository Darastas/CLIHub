use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, Write},
    path::Path,
};

use super::store::{Message, Store};

/// 运行 MCP Server 循环（基于 stdio）
pub fn run_mcp_server<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    room: impl AsRef<Path>,
    agent: &str,
) -> Result<()> {
    let room = room.as_ref().to_path_buf();
    let store = Store::new(&room)?;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {e}")
                    }
                });
                let _ = writeln!(writer, "{}", serde_json::to_string(&err_resp)?);
                let _ = writer.flush();
                continue;
            }
        };

        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // 处理各 MCP 方法
        let response: Option<Value> = match method {
            "initialize" => {
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "clihub-collab",
                            "version": "1.3.0"
                        }
                    }
                }))
            }
            "notifications/initialized" => {
                // 初始化完成通知，无需回复
                None
            }
            "ping" => {
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {}
                }))
            }
            "tools/list" => {
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            {
                                "name": "collab_get_task",
                                "description": "获取当前工作流分配给本 Agent 的最新协同任务说明、目标、角色专项规范与前序交接上下文",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "mark_read": {
                                            "type": "boolean",
                                            "description": "是否标记为已接收已读，默认为 true"
                                        }
                                    }
                                }
                            },
                            {
                                "name": "collab_finish_step",
                                "description": "完成当前阶段工作并向工作流交接成果，自动流转给下一位角色。若全部目标彻底达成且无需后续角色处理，设置 is_complete 为 true",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "summary": {
                                            "type": "string",
                                            "description": "工作成果总结、修改的代码说明以及交接给下一位角色的具体要求"
                                        },
                                        "is_complete": {
                                            "type": "boolean",
                                            "description": "是否已彻底完成整个用户任务目标（仅在确认无需其他角色继续开发/审查/验证时设为 true）"
                                        }
                                    },
                                    "required": ["summary"]
                                }
                            }
                        ]
                    }
                }))
            }
            "tools/call" => {
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                let tool_result = match tool_name {
                    "collab_get_task" => {
                        let mark_read = args.get("mark_read").and_then(|b| b.as_bool()).unwrap_or(true);
                        handle_get_task(&store, &room, agent, mark_read)
                    }
                    "collab_finish_step" => {
                        let summary = args.get("summary").and_then(|s| s.as_str()).unwrap_or("").trim();
                        let is_complete = args.get("is_complete").and_then(|b| b.as_bool()).unwrap_or(false);
                        if summary.is_empty() {
                            Err(anyhow!("summary 参数不能为空"))
                        } else {
                            handle_finish_step(&store, &room, agent, summary, is_complete)
                        }
                    }
                    _ => Err(anyhow!("未知工具: {tool_name}")),
                };

                match tool_result {
                    Ok(text) => {
                        Some(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [
                                    {
                                        "type": "text",
                                        "text": text
                                    }
                                ],
                                "isError": false
                            }
                        }))
                    }
                    Err(err) => {
                        Some(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [
                                    {
                                        "type": "text",
                                        "text": format!("执行出错: {err:#}")
                                    }
                                ],
                                "isError": true
                            }
                        }))
                    }
                }
            }
            _ => {
                if id.is_some() {
                    Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": format!("Method not found: {method}")
                        }
                    }))
                } else {
                    None
                }
            }
        };

        if let Some(resp) = response {
            let serialized = serde_json::to_string(&resp)?;
            writeln!(writer, "{}", serialized)?;
            writer.flush()?;
        }
    }

    Ok(())
}

fn handle_get_task(store: &Store, room: &Path, agent: &str, mark_read: bool) -> Result<String> {
    // 查找最近一条发给该 agent 的任务消息
    let messages = store.all_messages()?;
    let latest_msg = messages
        .iter()
        .rev()
        .find(|m| m.to == agent)
        .context("当前信箱中暂无分配给本 Agent 的任务")?;

    if mark_read {
        let _ = store.mark_read(agent, &latest_msg.id);
    }

    // 检查 tasks/ 目录下是否有更详细的 task markdown 文件
    let task_file = room.join("tasks").join(format!("{}.md", latest_msg.id));
    if task_file.is_file() {
        if let Ok(content) = fs::read_to_string(&task_file) {
            return Ok(content);
        }
    }

    // 否则直接返回消息正文与角色信息
    let round = store.load_round()?;
    let participant = round.participants.iter().find(|p| p.id == agent);
    let role_info = participant
        .map(|p| format!("你的角色: {} ({})", p.name, p.role))
        .unwrap_or_default();

    Ok(format!(
        "# 协同任务\n\n{}\n\n## 任务要求与前序输入\n{}",
        role_info, latest_msg.body
    ))
}

fn handle_finish_step(
    store: &Store,
    _room: &Path,
    agent: &str,
    summary: &str,
    is_complete: bool,
) -> Result<String> {
    let round = store.load_round()?;

    // 找到最近发给本 agent 的未回复请求 id 作为 reply_to
    let messages = store.all_messages()?;
    let latest_req = messages.iter().rev().find(|m| m.to == agent);
    let reply_to = latest_req.map(|m| m.id.clone());

    let mut body = summary.to_string();
    if is_complete {
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("\n[WORKFLOW_COMPLETE]");
    }

    let msg = Message {
        id: Store::new_id("m"),
        round: round.id,
        from: agent.to_string(),
        to: "user".to_string(),
        body,
        reply_to,
        anchor: None,
        snapshot: None,
        time: Store::now(),
    };

    let msg_id = msg.id.clone();
    store.send(msg)?;

    let status_hint = if is_complete {
        "已标记为全流程完成 [WORKFLOW_COMPLETE]，工作流将结束。"
    } else {
        "阶段成果已成功投递进信箱，下一位角色将自动接力。"
    };

    Ok(format!("交接成功 (消息 ID: {msg_id})。{status_hint}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::{Participant, Phase, Round};
    use std::io::Cursor;

    fn setup_test_room() -> (std::path::PathBuf, Store) {
        let tmp = std::env::temp_dir().join(Store::new_id("mcp-test"));
        let _ = fs::create_dir_all(&tmp);
        let store = Store::new(&tmp).unwrap();
        let round = Round {
            id: "r1".into(),
            title: "Test Round".into(),
            goal: "Test Goal".into(),
            repo: tmp.clone(),
            created: 1,
            phase: Phase::Development,
            participants: vec![
                Participant { id: "user".into(), name: "User".into(), role: "User".into() },
                Participant { id: "dev".into(), name: "Developer".into(), role: "主开发".into() },
            ],
        };
        store.create_round(round).unwrap();
        (tmp, store)
    }

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let (tmp, _store) = setup_test_room();
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n\
                     {\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n\
                     {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n";

        let reader = Cursor::new(input.as_bytes());
        let mut writer = Vec::new();

        run_mcp_server(reader, &mut writer, &tmp, "dev").unwrap();

        let output = String::from_utf8(writer).unwrap();
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        let init_resp: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "clihub-collab");

        let tools_resp: Value = serde_json::from_str(lines[1]).unwrap();
        let tools = tools_resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "collab_get_task");
        assert_eq!(tools[1]["name"], "collab_finish_step");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_mcp_tools_call_workflow() {
        let (tmp, store) = setup_test_room();
        // 先存入一条发给 dev 的请求
        store.send(Message {
            id: "req1".into(),
            round: "r1".into(),
            from: "user".into(),
            to: "dev".into(),
            body: "请实现登录逻辑".into(),
            reply_to: None,
            anchor: None,
            snapshot: None,
            time: 2,
        }).unwrap();

        let input = "{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"tools/call\",\"params\":{\"name\":\"collab_get_task\",\"arguments\":{\"mark_read\":true}}}\n\
                     {\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"tools/call\",\"params\":{\"name\":\"collab_finish_step\",\"arguments\":{\"summary\":\"登录模块实现完毕\",\"is_complete\":true}}}\n";

        let reader = Cursor::new(input.as_bytes());
        let mut writer = Vec::new();

        run_mcp_server(reader, &mut writer, &tmp, "dev").unwrap();

        let output = String::from_utf8(writer).unwrap();
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        let get_task_resp: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(get_task_resp["result"]["isError"], false);
        assert!(get_task_resp["result"]["content"][0]["text"].as_str().unwrap().contains("请实现登录逻辑"));

        // 验证已读标记
        assert!(store.is_read("dev", "req1"));

        let finish_resp: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(finish_resp["result"]["isError"], false);

        // 验证消息已入库，且包含 [WORKFLOW_COMPLETE]
        let all_msgs = store.all_messages().unwrap();
        assert_eq!(all_msgs.len(), 2);
        let reply = all_msgs.iter().find(|m| m.from == "dev").unwrap();
        assert!(reply.body.contains("登录模块实现完毕"));
        assert!(reply.body.contains("[WORKFLOW_COMPLETE]"));
        assert_eq!(reply.reply_to.as_deref(), Some("req1"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
