# Contrato JSONL

O modo JSON do Seshat Rust e ativado com:

```bash
seshat commit --format json
```

## Regras

- Cada evento e um objeto JSON em uma unica linha.
- Todo objeto tem o campo `event`.
- Eventos JSON sao escritos somente em stdout.
- Logs humanos, avisos e erros de diagnostico ficam em stderr quando JSON mode esta ativo.
- Erros preservam exit code diferente de zero.

## Eventos

### `message_ready`

Emitido quando a mensagem de commit foi calculada.

```json
{"event":"message_ready","message":"docs: update README.md"}
```

Campos:

- `event`: sempre `message_ready`.
- `message`: mensagem Conventional Commit final.

### `committed`

Emitido depois que `git commit` conclui com sucesso.

```json
{"event":"committed","summary":"abc1234 docs: update README.md"}
```

Com `--date`:

```json
{"event":"committed","summary":"abc1234 docs: update README.md","date":"2020-01-02"}
```

Campos:

- `event`: sempre `committed`.
- `summary`: resumo retornado pelo ultimo commit Git.
- `date`: presente somente quando a data foi definida por flag ou configuracao efetiva.

### `cancelled`

Emitido quando a mensagem foi gerada, mas o commit nao foi confirmado.

```json
{"event":"cancelled","reason":"user_declined"}
```

Campos:

- `event`: sempre `cancelled`.
- `reason`: motivo estavel do cancelamento.

### `error`

Emitido quando o comando falha.

```json
{"event":"error","message":"Arquivo .seshat não encontrado. O Seshat requer um arquivo de configuração .seshat no projeto."}
```

Campos:

- `event`: sempre `error`.
- `message`: erro legivel para humanos.

## Eventos Reservados

Os nomes `review_ready` e `check_result` ficam reservados para evolucao futura. Eles nao sao emitidos pela implementacao atual.
