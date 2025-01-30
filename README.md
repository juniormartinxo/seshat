# Seshat 🤖

CLI para automação de commits usando Conventional Commits com suporte a múltiplos provedores de IA (DeepSeek, Claude e Ollama)

![Python](https://img.shields.io/badge/Python-3.8%2B-blue)
![Git](https://img.shields.io/badge/Git-Integrado-green)
![License](https://img.shields.io/badge/License-MIT-orange)

## 📦 Instalação

```bash
# Instalação via pip (recomendado)
pip install git+https://github.com/juniormartinxo/seshat.git

# Para desenvolvimento local
git clone https://github.com/juniormartinxo/seshat.git
cd seshat
pip install -e .
```

## ⚙️ Configuração

### 1. Configuração do Provedor e API Key

Você pode configurar o Seshat usando o comando `config`:

```bash
# Configurar provedor
seshat config --provider deepseek  # ou claude/ollama

# Configurar API Key (para DeepSeek ou Claude)
seshat config --api-key SUA_CHAVE_API

# Verificar configuração atual
seshat config
```

### 2. Configuração Alternativa
Alternativamente, você pode configurar através de variáveis de ambiente ou arquivo `.env`:

```bash
# Via variáveis de ambiente
export AI_PROVIDER=deepseek  # ou claude/ollama
export API_KEY=sua_chave_aqui

# Ou via arquivo .env
AI_PROVIDER=deepseek
API_KEY=sua_chave_aqui
```

## 🚀 Uso

### Commit Básico
```bash
git add .
seshat commit
```

### Opções Avançadas
```bash
seshat commit \
  --provider claude \  # Força um provedor específico
  --model claude-3-haiku-20240307 \  # Define modelo específico
  --yes \  # Pula confirmação
  --verbose  # Exibe detalhes do processo
```

## 🛠️ Funcionalidades

### Provedores de IA Suportados
- **DeepSeek**: Integração via API DeepSeek
- **Claude**: Integração via API Anthropic
- **Ollama**: Integração local com modelos do Ollama

### Configuração do Ollama
Para usar o Ollama como provedor:

1. Instale o Ollama: https://ollama.ai
2. Inicie o serviço: `ollama serve`
3. Baixe o modelo padrão: `ollama pull deepseek-coder-v2`
4. Configure o Seshat: `seshat config --provider ollama`

### Validações e Segurança
- Verificação de arquivos staged antes do commit
- Validação do tamanho do diff (alertas em 2500 caracteres, limite em 3000)
- Confirmação interativa das mensagens geradas
- Validação do formato Conventional Commits

### Tipos de Commit Suportados
- `feat`: Nova funcionalidade
- `fix`: Correção de bug
- `docs`: Alterações na documentação
- `style`: Mudanças de formatação
- `refactor`: Refatoração de código
- `perf`: Melhorias de performance
- `test`: Adição/ajuste de testes
- `chore`: Tarefas de manutenção
- `build`: Mudanças no sistema de build
- `ci`: Mudanças na CI/CD
- `revert`: Reversão de commit

## 📚 Arquitetura

```text
seshat/
├── cli.py         # Interface de linha de comando e comandos
├── core.py        # Lógica central, validações e integração Git
├── providers.py   # Implementação dos provedores de IA
└── utils.py       # Utilitários e gerenciamento de configuração
```

## ⚠️ Requisitos

- Python 3.8+
- Git instalado
- Para DeepSeek/Claude: Chave de API válida
- Para Ollama: Servidor Ollama local

## 🔍 Troubleshooting

### Erros Comuns

1. **Configuração Inválida**
```bash
# Verifique a configuração atual
seshat config

# Reconfigure se necessário
seshat config --provider deepseek
seshat config --api-key NOVA_CHAVE
```

2. **Erro com Ollama**
```bash
# Verifique se o servidor está rodando
curl http://localhost:11434/api/version

# Verifique se o modelo está instalado
ollama list
```

3. **Diff muito grande**
```bash
# Divida suas alterações em commits menores
git add -p  # Adicione alterações interativamente
```

## 📝 Licença

MIT © [Junior Martins](https://github.com/juniormartinxo)

---

## 🤝 Contribuição

1. Fork o projeto
2. Crie sua branch (`git checkout -b feature/AmazingFeature`)
3. Commit suas mudanças (`seshat commit`)
4. Push para a branch (`git push origin feature/AmazingFeature`)
5. Abra um Pull Request

---

🐛 [Reportar Bug](https://github.com/juniormartinxo/seshat/issues)  
✨ [Sugerir Funcionalidade](https://github.com/juniormartinxo/seshat/issues)