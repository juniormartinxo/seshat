# Seshat 🤖

CLI para automação de commits usando Conventional Commits com suporte a múltiplos provedores de IA (DeepSeek e Claude)

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

### 1. Defina o Provedor de IA
Configure o provedor desejado através da variável de ambiente `AI_PROVIDER`:

```bash
# Via arquivo .env
AI_PROVIDER=deepseek  # ou claude
```

### 2. Configure sua API Key
Você tem três opções para configurar sua chave de API:

```bash
# 1. Via comando (recomendado)
seshat config --api-key SUA_CHAVE_API

# 2. Via variável de ambiente
export API_KEY="sua_chave_aqui"

# 3. Via arquivo .env
API_KEY=sua_chave_aqui
```

### 3. Modelo de IA (Opcional)
Defina um modelo específico do provedor escolhido:

```bash
# Via arquivo .env
AI_MODEL=deepseek-chat  # para DeepSeek
AI_MODEL=claude-3-haiku-20240307  # para Claude
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
- **DeepSeek**: Provedor padrão
- **Claude**: Alternativa via API da Anthropic

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
├── cli.py         # Interface de linha de comando
├── core.py        # Lógica central e integração com Git
├── providers.py   # Implementação dos provedores de IA
└── utils.py       # Utilitários e configurações
```

## ⚠️ Requisitos

- Python 3.8+
- Git instalado
- Conta em um dos provedores suportados (DeepSeek ou Anthropic)
- Chave de API válida

## 🔍 Troubleshooting

### Erros Comuns

1. **API Key não encontrada**
```bash
# Verifique a configuração atual
seshat config

# Reconfigure se necessário
seshat config --api-key NOVA_CHAVE
```

2. **Provedor Inválido**
```bash
# Certifique-se que AI_PROVIDER está configurado corretamente
echo $AI_PROVIDER
# Deve retornar 'deepseek' ou 'claude'
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