//! Servidor MCP por stdio, em JSON-RPC mínimo à mão (D11 da TASK; spec §9.4).
//!
//! O protocolo e o contrato das quatro ferramentas vivem aqui; quem executa uma ferramenta é um
//! [`Backend`], que roda o mesmo comando do CLI, então a saída é idêntica à do shell.

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

pub struct Server<B: Backend> {
    backend: B,
    repo: Option<String>,
}

impl<B: Backend> Server<B> {
    /// `repo` vira `--repo <caminho>` em todo comando executado.
    pub fn new(backend: B, repo: Option<String>) -> Self {
        let _ = (&backend, &repo);
        todo!("spec §9.4")
    }

    /// Trata uma linha JSON-RPC; `None` para notificações, que não têm resposta.
    pub fn handle(&self, _line: &str) -> Option<String> {
        let _ = (&self.backend, &self.repo);
        todo!("spec §9.4")
    }

    /// Lê uma mensagem por linha até o fim da entrada e escreve uma resposta por linha.
    pub fn serve(&self, _input: impl BufRead, _output: impl Write) -> std::io::Result<()> {
        todo!("spec §9.4")
    }
}
