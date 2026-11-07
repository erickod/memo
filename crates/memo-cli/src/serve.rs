//! `memo serve`: o servidor MCP roda os mesmos comandos do CLI, em processo.

use crate::{Cli, Failure, execute};
use memo_mcp::{Backend, Server, ToolResult};

/// Executa cada ferramenta como o comando equivalente, então a saída é idêntica à do shell.
struct InProcess;

impl Backend for InProcess {
    fn run(&self, argv: &[String]) -> ToolResult {
        let mut args = vec!["memo".to_string()];
        args.extend(argv.iter().cloned());
        let outcome = execute(&args);
        let text = if outcome.stdout.is_empty() {
            outcome.stderr
        } else {
            outcome.stdout
        };
        ToolResult {
            text,
            is_error: outcome.code != 0,
        }
    }
}

/// Fala MCP por stdio até o fim da entrada.
pub fn serve(cli: &Cli) -> Result<(), Failure> {
    let repo = cli.repo.as_ref().map(|p| p.display().to_string());
    let server = Server::new(InProcess, repo);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    server
        .serve(stdin.lock(), stdout.lock())
        .map_err(|e| Failure::Internal(e.to_string()))
}
