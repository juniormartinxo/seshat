# Backlog de Refatoracao e Migracao

Este diretorio transforma `docs/refactor-plan.md` em cards executaveis.

## Como Usar

- Execute os cards em ordem numerica salvo quando as dependencias indicarem o contrario.
- Cada card deve resultar em um PR pequeno ou um commit coeso.
- Antes de iniciar um card, confirme que suas dependencias estao concluidas.
- Ao terminar, rode a validacao indicada no card.
- Mantenha `docs/refactor-plan.md` como plano macro e estes cards como plano de execucao.

## Convencoes

- `Status`: `todo`, `doing`, `blocked`, `done`.
- `Priority`: `P0` bloqueia a migracao, `P1` fecha paridade essencial, `P2` melhora UX/manutencao.
- `Type`: `refactor`, `test`, `feature`, `docs`, `release`.
- `Owner`: vazio por padrao.
- `Dependencies`: lista de cards que devem estar prontos antes.

## Sequencia Recomendada

1. `001-parity-matrix.md`
2. `002-git-client-repo-path.md`
3. `003-e2e-git-test-harness.md`
4. `004-e2e-no-ai-commit-fast-paths.md`
5. `005-e2e-cli-init-config-fix.md`
6. `006-dotenv-support.md`
7. `007-keyring-secret-storage.md`
8. `008-effective-config-pipeline.md`
9. `009-provider-http-abstraction.md`
10. `010-openai-compatible-provider-tests.md`
11. `011-anthropic-gemini-ollama-tests.md`
12. `012-cli-provider-tests.md`
13. `013-code-review-judge-flow.md`
14. `014-review-logging-filtering.md`
15. `015-tooling-refactor-strategies.md`
16. `016-tooling-e2e-fake-commands.md`
17. `017-ui-output-contract.md`
18. `018-json-mode-contract.md`
19. `019-gpg-hardening.md`
20. `020-release-packaging-docs.md`
21. `021-python-cutover.md`

## Milestones

### Milestone 1: Git Seguro

- `001-parity-matrix.md`
- `002-git-client-repo-path.md`
- `003-e2e-git-test-harness.md`
- `004-e2e-no-ai-commit-fast-paths.md`

### Milestone 2: Config Confiavel

- `005-e2e-cli-init-config-fix.md`
- `006-dotenv-support.md`
- `007-keyring-secret-storage.md`
- `008-effective-config-pipeline.md`

### Milestone 3: IA Testavel

- `009-provider-http-abstraction.md`
- `010-openai-compatible-provider-tests.md`
- `011-anthropic-gemini-ollama-tests.md`
- `012-cli-provider-tests.md`

### Milestone 4: Review Completo

- `013-code-review-judge-flow.md`
- `014-review-logging-filtering.md`

### Milestone 5: CLI Pronta para Uso Diario

- `015-tooling-refactor-strategies.md`
- `016-tooling-e2e-fake-commands.md`
- `017-ui-output-contract.md`
- `018-json-mode-contract.md`
- `019-gpg-hardening.md`

### Milestone 6: Corte Final

- `020-release-packaging-docs.md`
- `021-python-cutover.md`
