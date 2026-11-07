//! Servidor MCP por stdio, em JSON-RPC mínimo à mão (D11 da TASK; spec §9.4).
//!
//! O protocolo e o contrato das quatro ferramentas vivem aqui; quem executa uma ferramenta é um
//! [`Backend`], que roda o mesmo comando do CLI, então a saída é idêntica à do shell.

use serde_json::{Map, Value, json};
use std::io::{BufRead, Write};

/// Resultado de uma ferramenta: o texto compacto e se foi erro (`isError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
}

/// Executa um comando do `memo` (argv sem o nome do programa) e devolve a saída.
pub trait Backend {
    fn run(&self, argv: &[String]) -> ToolResult;
}

const DEFAULT_PROTOCOL: &str = "2024-11-05";
const QUERY_KINDS: &[&str] = &[
    "symbol", "callers", "callees", "impact", "writers", "readers", "type", "tests", "word",
];
const NOTE_TYPES: &[&str] = &["decisao", "regra", "restricao", "descartado", "glossario"];

pub struct Server<B: Backend> {
    backend: B,
    repo: Option<String>,
}

fn reply(id: &Value, body: Value) -> String {
    let mut message = json!({ "jsonrpc": "2.0", "id": id });
    if let (Some(obj), Some(extra)) = (message.as_object_mut(), body.as_object()) {
        obj.extend(extra.clone());
    }
    message.to_string()
}

fn error(id: &Value, code: i64, message: &str) -> String {
    reply(id, json!({ "error": { "code": code, "message": message } }))
}

/// As quatro ferramentas da spec §9.4, com descrições curtas para gastar pouco contexto.
fn tools() -> Value {
    json!([
        {
            "name": "brief",
            "description": "Briefing estrutural para um pedido: âncoras, símbolos, contorno, escritores e notas.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "O pedido em texto livre." },
                    "task": { "type": "string", "description": "Id da task (deltas e notas da task)." }
                },
                "required": ["text"]
            }
        },
        {
            "name": "query",
            "description": "Consulta ao grafo: symbol, callers, callees, impact, writers, readers, type, tests ou word.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": QUERY_KINDS },
                    "target": { "type": "string", "description": "Símbolo (qname ou sufixo único), atributo, tipo ou palavra." },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 3, "description": "Só para callers e callees." },
                    "task": { "type": "string" }
                },
                "required": ["kind", "target"]
            }
        },
        {
            "name": "note_add",
            "description": "Registra uma nota atômica ancorada num símbolo, palavra, aresta ou arquivo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tipo": { "type": "string", "enum": NOTE_TYPES },
                    "ancora": { "type": "string", "description": "sym:<qname>, word:<texto>, edge:<a>-><b> ou file:<caminho>." },
                    "texto": { "type": "string", "description": "Uma afirmação, até 1.200 caracteres." },
                    "task": { "type": "string" },
                    "confirm": { "type": "boolean", "description": "Só para decisão vinda de rodada humana." }
                },
                "required": ["tipo", "ancora", "texto"]
            }
        },
        {
            "name": "status",
            "description": "Base, features, notas por estado e tasks promovíveis.",
            "inputSchema": {
                "type": "object",
                "properties": { "task": { "type": "string" } }
            }
        }
    ])
}

fn required<'a>(args: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or invalid argument '{key}'"))
}

fn optional<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

impl<B: Backend> Server<B> {
    /// `repo` vira `--repo <caminho>` em todo comando executado.
    pub fn new(backend: B, repo: Option<String>) -> Self {
        Self { backend, repo }
    }

    /// Argv do comando do CLI que equivale à ferramenta.
    fn argv(&self, tool: &str, args: &Map<String, Value>) -> Result<Vec<String>, String> {
        let mut argv: Vec<String> = Vec::new();
        if let Some(repo) = &self.repo {
            argv.extend(["--repo".to_string(), repo.clone()]);
        }
        if let Some(task) = optional(args, "task") {
            argv.extend(["--task".to_string(), task.to_string()]);
        }
        let push = |argv: &mut Vec<String>, items: &[&str]| {
            argv.extend(items.iter().map(|s| (*s).to_string()));
        };
        match tool {
            "brief" => push(&mut argv, &["brief", "--", required(args, "text")?]),
            "query" => {
                let kind = required(args, "kind")?;
                if !QUERY_KINDS.contains(&kind) {
                    return Err(format!("unknown query kind '{kind}'"));
                }
                push(&mut argv, &["query", kind]);
                let depth = args.get("depth").and_then(Value::as_i64);
                if let (Some(depth), "callers" | "callees") = (depth, kind) {
                    push(&mut argv, &["--depth", &depth.to_string()]);
                }
                push(&mut argv, &["--", required(args, "target")?]);
            }
            "note_add" => {
                let (tipo, ancora, texto) = (
                    required(args, "tipo")?,
                    required(args, "ancora")?,
                    required(args, "texto")?,
                );
                push(
                    &mut argv,
                    &["note", "add", "--tipo", tipo, "--ancora", ancora],
                );
                push(&mut argv, &["--origem", "agente"]);
                if args.get("confirm").and_then(Value::as_bool) == Some(true) {
                    push(&mut argv, &["--confirm"]);
                }
                push(&mut argv, &["--", texto]);
            }
            "status" => push(&mut argv, &["status"]),
            other => return Err(format!("unknown tool '{other}'")),
        }
        Ok(argv)
    }

    fn call_tool(&self, id: &Value, params: &Value) -> String {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let empty = Map::new();
        let args = params
            .get("arguments")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        match self.argv(name, args) {
            Err(message) => error(id, -32602, &message),
            Ok(argv) => {
                let result = self.backend.run(&argv);
                reply(
                    id,
                    json!({ "result": {
                        "content": [{ "type": "text", "text": result.text }],
                        "isError": result.is_error
                    }}),
                )
            }
        }
    }

    /// Trata uma linha JSON-RPC; `None` para notificações, que não têm resposta.
    pub fn handle(&self, line: &str) -> Option<String> {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return Some(error(&Value::Null, -32700, "Parse error"));
        };
        let Some(object) = message.as_object() else {
            return Some(error(&Value::Null, -32600, "Invalid Request"));
        };
        let id = object.get("id")?;
        let method = object.get("method").and_then(Value::as_str);
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            None => error(id, -32600, "Invalid Request"),
            Some("initialize") => {
                let version = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PROTOCOL);
                reply(
                    id,
                    json!({ "result": {
                        "protocolVersion": version,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "memo", "version": env!("CARGO_PKG_VERSION") }
                    }}),
                )
            }
            Some("ping") => reply(id, json!({ "result": {} })),
            Some("tools/list") => reply(id, json!({ "result": { "tools": tools() } })),
            Some("tools/call") => self.call_tool(id, &params),
            Some(_) => error(id, -32601, "Method not found"),
        })
    }

    /// Lê uma mensagem por linha até o fim da entrada e escreve uma resposta por linha.
    pub fn serve(&self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(response) = self.handle(&line) {
                writeln!(output, "{response}")?;
                output.flush()?;
            }
        }
        Ok(())
    }
}
