## 1) O que o Tauri v2 será no seu produto

**Tauri = Shell + UI + Permissões + IPC (ponte)**

* UI (frontend) renderiza cockpit, grafo, relatórios
* Backend (Rust sidecar / plugins) faz:

  * leitura do repo
  * geração de IR
  * diff semântico
  * policy checks
  * evidence runner (parcial)
  * orquestração de tarefas LLM
  * controle do sandbox local

**Regra de design**:

> o “core” não fica espalhado na UI.
> A UI só chama comandos “seguro e tipado”.

---

## 2) Arquitetura técnica (Tauri-first)

### 2.1 Componentes

**A) Tauri App**

* `Frontend` (React/Vue/Svelte) → cockpit
* `Tauri Core (Rust)` → comandos e segurança
* `Plugins`:

  * filesystem (com allowlist)
  * process (exec controlado)
  * shell (restrito)
  * updater (se necessário)
* `Sidecars` opcionais:

  * `igdp-core` (binário) se você quiser separar lógica pesada
  * `sandbox-runner` (binário) para executar tarefas isoladas

**B) IGDP Core (Rust)**

* Parser do `intent.yaml`
* IR builder (`system_twin.json`)
* Semantic diff builder (`semantic_diff.json`)
* Policy engine (`policy_report.json`)
* Evidence planner e runner incremental (gerar `evidence_report.json`)
* Planner (`action_plan.json`)
* Task orchestrator (LLM tasks)

**C) Sandbox Runner**

* MVP: subprocess isolado + regras de path + no-network (dependendo do OS)
* depois: container (Docker) ou sandbox de VM leve
* o Tauri só manda “tasks”, o runner executa e devolve artefatos

---

## 3) Como fica o fluxo do usuário com Tauri

### “Abrir repo”

* UI → `openRepository(path)`
* Core valida e monta o “workspace”
* Se não existir `intent.yaml`, oferece “Bootstrap”

### “Atualizar em tempo real”

Tauri v2: use watchers do Rust para:

* monitorar mudanças no repo (`git diff`, file watcher)
* regenerar IR incrementalmente
* atualizar UI via eventos (`emit`)

**Resultado**: cockpit vivo (o que você queria).

---

## 4) Segurança no Tauri (muito importante)

Tauri é ótimo porque te força a pensar em permissões.

### 4.1 File system allowlist

* a app só acessa o diretório do repo aberto
* bloqueia `.ssh`, `~/.config`, etc.

### 4.2 Exec controlado

* Core tem uma lista de comandos permitidos:

  * `pytest`, `npm test`, `pnpm test`
  * `igdp policy_check`
* nada de “rodar qualquer coisa”

### 4.3 LLM tasks só via patch

* a UI nunca aplica patch diretamente
* Core valida:

  * limite de arquivos alterados
  * paths permitidos
  * tamanho do diff
  * proibição de tocar em `.github/workflows` sem permissão explícita

---

## 5) UI stack dentro do Tauri (recomendação prática)

Você tem duas opções boas:

### Opção 1 — React + Vite

* simples, rápido, maduro
* ótimo para webview no desktop

### Opção 2 — SvelteKit

* mais leve e rápido para UI “cockpit”
* excelente para muitos painéis e reatividade

Para o grafo: use uma lib de graph decente (ex.: Cytoscape.js, React Flow, etc.) e trate o “System Twin” como data source.

---

## 6) Como integrar Git/CI sem quebrar tudo (com Tauri no centro)

### Local (Tauri app)

* gera os artefatos localmente
* mostra o cockpit
* roda evidence incremental local
* permite “corrigir policy” e “gerar teste faltante” via tasks

### CI

* repete o mesmo pipeline com `igdp-core` (CLI)
* garante consistência entre o que o dev viu e o que a CI valida

**Chave**: o core precisa existir como **CLI + library**.

* Tauri chama a library
* CI chama o CLI
* mesma lógica, mesmos outputs JSON

---

## 7) Como organizar o monorepo do produto (sugestão)

```
igdp/
  apps/
    desktop-tauri/
      src-tauri/        # rust commands
      src/              # ui
  crates/
    igdp_core/          # rust lib: IR, diff, policy, planner
    igdp_cli/           # binary: gera jsons, roda checks
    igdp_sandbox/       # runner isolado (bin)
  schemas/
    intent.schema.json
    system_twin.schema.json
    semantic_diff.schema.json
    policy_report.schema.json
    evidence_report.schema.json
```

---

## 8) MVP recomendado com Tauri (sem se matar)

### MVP 1 (2–4 semanas, dependendo do time)

* Abrir repo
* Ler `intent.yaml`
* Gerar `system_twin.json`
* Gerar `semantic_diff.json` vs base branch
* Rodar policies (as 10 iniciais)
* Mostrar cockpit + policy view + diff view
* Botão “Run Evidence” rodando unit/contract mínimos
* Exportar artefatos

### MVP 2

* Planner + auto-fix patches (redaction/timeout/auth)
* LLM tasks estruturadas para gerar testes faltantes
* Sandbox runner básico

---

## 9) O que eu preciso te avisar (sinceridade técnica)

* Tauri é perfeito para UI/IPC, mas **não é sandbox** por si só.
* O “sandbox runner” precisa existir (mesmo que simples no MVP).
* A consistência “local vs CI” só fica forte se o core virar **CLI**.

---


