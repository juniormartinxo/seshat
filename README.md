# Seshat 🤖

CLI para automação de commits usando Conventional Commits com suporte a múltiplos provedores de IA.

![Python](https://img.shields.io/badge/Python-3.8%2B-blue)
![Git](https://img.shields.io/badge/Git-Integrado-green)
![License](https://img.shields.io/badge/License-MIT-orange)

## Instalação

### Via pipx (recomendado)
```bash
# Instalar pipx se necessário
python -m pip install --user pipx
python -m pipx ensurepath

# Instalar Seshat
pipx install git+https://github.com/juniormartinxo/seshat.git
```

### Desenvolvimento
```bash
git clone https://github.com/juniormartinxo/seshat.git
cd seshat
pip install -e .
```

## Configuração

### Provedores de IA Suportados

- DeepSeek API
- Claude API (Anthropic)
- Ollama (local)

### API Key e Provider

Via CLI:
```bash
seshat config --provider deepseek|claude|ollama
seshat config --api-key SUA_CHAVE_API
seshat config --model seu-modelo
```

Via `.env`:
```bash
AI_PROVIDER=deepseek|claude|ollama
API_KEY=sua_chave_aqui
AI_MODEL=seu-modelo
```

### Configuração do Ollama

1. Instale o [Ollama](https://ollama.ai)
2. Inicie o servidor: `ollama serve`
3. Baixe o modelo: `ollama pull deepseek-coder-v2`
4. Configure: `seshat config --provider ollama`

## Uso

Commit básico:
```bash
git add .
seshat commit
```

Opções avançadas:
```bash
seshat commit \
  --provider claude \
  --model claude-3-haiku-20240307 \
  --yes \
  --verbose
```

## Recursos

- 3 provedores de IA suportados
- Validação de tamanho do diff (alertas em 2500 caracteres)
- Verificação de arquivos staged
- Suporte completo ao Conventional Commits
- Confirmação interativa das mensagens

## Tipos de Commit

- `feat`: Nova funcionalidade
- `fix`: Correção de bug
- `docs`: Documentação
- `style`: Formatação
- `refactor`: Refatoração
- `perf`: Performance
- `test`: Testes
- `chore`: Manutenção
- `build`: Build
- `ci`: CI/CD
- `revert`: Reversão

## Requisitos

- Python 3.8+
- Git
- API Key (DeepSeek/Claude)
- Ollama (opcional)

## Solução de Problemas

### Configuração

```bash
# Verificar config
seshat config

# Reconfigurar
seshat config --provider deepseek
seshat config --api-key NOVA_CHAVE
```

### Ollama

```bash
# Verificar servidor
curl http://localhost:11434/api/version

# Verificar modelos
ollama list
```

### Diff Grande
```bash
# Dividir alterações
git add -p
```

## Licença

MIT © [Junior Martins](https://github.com/juniormartinxo)

---
🐛 [Reportar Bug](https://github.com/juniormartinxo/seshat/issues)
✨ [Sugerir Feature](https://github.com/juniormartinxo/seshat/issues)
