## 21) LLM Orchestrator — o que ele é (e o que ele não é)

### O que é

Um **motor de execução de tarefas** (não um chat) que:

* recebe um `action_plan.json`
* escolhe: **patch determinístico** ou **LLM task**
* aplica mudanças sempre como **patch auditável**
* roda validações rápidas antes de “devolver” ao humano

### O que não é

* não é “autonomia total”
* não é “editar repo livremente”
* não é “executar comandos aleatórios”

Regra de ouro:

> **A LLM propõe. O sistema aplica. O gate decide. O humano governa.**

---

## 22) Protocolo de tarefas LLM: “Task Protocol v0.1”

A LLM só trabalha dentro de um envelope padronizado.

### 22.1 Estrutura de input (o sistema manda pra LLM)

```json
{
  "task": {
    "task_id": "TASK-123",
    "type": "fix_policy_violation",
    "goal": "Make policy SEC-002 pass without changing business behavior",
    "constraints": {
      "max_files_changed": 3,
      "no_network": true,
      "no_new_dependencies": true,
      "must_keep_api_unchanged": true
    },
    "context": {
      "intent_excerpt": {
        "policies": ["SEC-002"],
        "data_sensitive_fields": ["card_last4"]
      },
      "violation": {
        "file": "src/payments/logger.ts",
        "line": 88,
        "details": {"keys": ["card_last4"]}
      },
      "repo_map": {
        "language": "typescript",
        "framework": "node",
        "test": "jest"
      }
    },
    "allowed_tools": ["read_file", "search_repo", "propose_patch", "run_tests"]
  }
}
```

O detalhe importante: **contexto mínimo** e **restrições explícitas**.

---

### 22.2 Estrutura de output (a LLM precisa devolver)

```json
{
  "task_id": "TASK-123",
  "result": {
    "status": "proposed",
    "summary": "Redacted card_last4 in logger payload",
    "patch": {
      "format": "git_unified_diff",
      "diff": "diff --git a/src/payments/logger.ts b/src/payments/logger.ts\n..."
    },
    "explanations": {
      "what_changed": ["Replaced card_last4 with redact(card_last4) in log payload"],
      "why_safe": ["Does not affect business logic, only logging output"],
      "how_to_verify": ["Run: npm test", "Run policy scan: igdp policy_check"]
    },
    "risk_notes": ["None"]
  }
}
```

Regras rígidas:

* sem patch = sem mudança
* sem justificativa = não aplica
* se exceder `max_files_changed` = rejeita automaticamente

---

## 23) Ferramentas permitidas (capabilities)

A LLM não recebe um shell livre. Ela recebe um “MCP interno” simples:

* `read_file(path, start_line, end_line)`
* `search_repo(query)`
* `list_files(glob)`
* `propose_patch(diff)`
* `run_tests(scope)` *(por exemplo: unit, contract, impacted)*
* `run_policy_check(policy_id)`
* `explain_semantic_impact()` *(opcional, baseado em IR)*

Nada de:

* `curl`
* instalar dependência
* acessar secrets
* rodar deploy

---

## 24) Sandbox Runner — isolamento de verdade (MVP e depois)

### 24.1 MVP (prático e suficiente)

* container por tarefa
* repo montado como volume (read-write)
* **rede desativada** por padrão
* variáveis de ambiente saneadas (sem secrets)
* limites:

  * CPU
  * memória
  * tempo (timeout)
* logs e artefatos coletados

### 24.2 MVP+ (quando ficar sério)

* filesystem “overlay” (mudança só vira patch)
* **denylist/allowlist de paths**

  * ex.: LLM não pode tocar `.github/workflows/` sem permissão
* execução de testes em modo restrito

---

## 25) Ciclo completo: “mudança → gate”

Aqui está o loop operacional que evita bagunça:

1. **Planner** gera `action_plan.json`
2. Orchestrator executa Step 1:

   * se `autofix_patch`: aplica patch determinístico
   * se `llm_task`: chama LLM e valida output
3. **Pre-flight check** (rápido):

   * lint/typecheck mínimo
   * policy check afetado
4. Se ok, executa Step 2, Step 3...
5. Ao final, roda `evidence_check incremental`
6. Gera:

   * `policy_report.json`
   * `evidence_report.json`
   * `semantic_diff.json`
7. UI atualiza e o humano decide

O humano não perde tempo “lendo código”; ele só olha:

* **o que mudou**
* **o que ficou mais arriscado**
* **qual evidência prova que está ok**

---

## 26) Anti-loop infinito (essencial)

LLM tentando corrigir algo e piorando é comum. Então:

* máximo de 2 tentativas por task
* se falhar 2x:

  * Planner reclassifica como “Human Attention”
  * gera uma explicação curta: “por que falhou” + “o que você precisa decidir”
* nada de “agentic spiral”

---

## 27) Integração com PR (como isso aparece no GitHub/GitLab)

### Comentário automático do PR (padrão)

* resumo semântico
* gate status
* top 3 riscos
* lista de blockers + botões/links para IDE cockpit

Exemplo (conceitual):

* ✅ Contracts: pass
* ❌ Policies: SEC-002 fail (PII in logs)
* ⚠ Missing evidence: INV-002 property test

E link para abrir a tela exata do cockpit já focada no problema.

---

## 28) Como isso começa simples (realista) sem “revolução forçada”

**Fase 0**: só CI artefatos + comentário semântico
**Fase 1**: plugin VS Code com cockpit (renderiza os JSONs)
**Fase 2**: adiciona Orchestrator local (tarefas simples: gerar testes, aplicar patch)
**Fase 3**: IDE cockpit completa + sandbox robusto

Você não troca o mundo de uma vez. Você adiciona **governança** em camadas.

---

