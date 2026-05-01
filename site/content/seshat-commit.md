# Modelo `seshat-commit`

`seshat-commit` e um modelo Ollama treinado para gerar mensagens de commit no padrao Conventional Commits, com foco em PT-BR.

Pagina publica:

- https://ollama.com/juniormartinxo/seshat-commit

## Requisitos

- Ollama instalado e rodando.
- Seshat configurado para usar provider `ollama`.

## Baixar do Ollama

```bash
ollama pull juniormartinxo/seshat-commit
```

Verifique se o modelo ficou disponivel:

```bash
ollama list
```

## Usar direto no Ollama

Com um diff staged:

```bash
git diff --cached | ollama run juniormartinxo/seshat-commit
```

Com um diff qualquer em arquivo:

```bash
ollama run juniormartinxo/seshat-commit < diff.patch
```

O modelo deve responder apenas com a mensagem de commit.

## Configurar no Seshat

Configure o Seshat para usar o modelo publicado:

```bash
seshat config --provider ollama --model juniormartinxo/seshat-commit
```

Depois use o fluxo normal:

```bash
seshat commit --yes
```

Ou por arquivo:

```bash
seshat flow 3 --yes
```

## Usar um modelo local treinado

Se voce treinou e gerou um GGUF local com o pipeline de `scripts/training`, importe no Ollama:

```bash
cd scripts/training
./import_to_ollama.sh out/seshat-commit-*/gguf/*.Q4_K_M.gguf seshat-commit
```

Configure o Seshat para usar o nome local:

```bash
seshat config --provider ollama --model seshat-commit
```

Teste direto:

```bash
git diff --cached | ollama run seshat-commit
```

## Publicar ou compartilhar no Ollama

Depois de importar o modelo local, copie para o namespace da sua conta:

```bash
ollama cp seshat-commit SEU_USUARIO/seshat-commit
```

Publique:

```bash
ollama push SEU_USUARIO/seshat-commit
```

Com tag:

```bash
ollama cp seshat-commit SEU_USUARIO/seshat-commit:v1
ollama push SEU_USUARIO/seshat-commit:v1
```

Outras pessoas podem baixar com:

```bash
ollama pull SEU_USUARIO/seshat-commit
```

E usar no Seshat com:

```bash
seshat config --provider ollama --model SEU_USUARIO/seshat-commit
```

Antes de publicar, revise se o dataset e o Modelfile nao contem dados sensiveis.

## Problemas comuns

Se o Seshat disser que o modelo nao existe:

```bash
ollama list
```

Confirme se o nome configurado no Seshat e exatamente o mesmo nome mostrado pelo Ollama.

Se estiver usando o modelo publicado, use o nome completo:

```text
juniormartinxo/seshat-commit
```

Se estiver usando o modelo local importado, use o nome local:

```text
seshat-commit
```
