# Seshat 🤖

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/juniormartinxo/seshat) ![Python](https://img.shields.io/badge/Python-3.8%2B-blue)
![Git](https://img.shields.io/badge/Git-Integrado-green)
![License](https://img.shields.io/badge/License-MIT-orange)

Uma CLI poderosa para automatizar a criação de mensagens de commit seguindo o padrão Conventional Commits, utilizando o poder da Inteligência Artificial.

## ✨ Recursos

*   ✅ **Múltiplos Provedores de IA:** Suporte para DeepSeek API, Claude API (Anthropic) e Ollama (local).
*   📏 **Validação de Tamanho do Diff:**  Alertas para diffs grandes (acima de 2500 caracteres), incentivando commits menores e mais focados.
*   🔍 **Verificação de Arquivos Staged:** Garante que você não se esqueça de adicionar arquivos ao commit.
*   📝 **Suporte Completo a Conventional Commits:**  Gera mensagens de commit padronizadas e significativas.
*   🤝 **Confirmação Interativa:**  Permite revisar e editar a mensagem de commit gerada pela IA antes de confirmar.
*   ⚙️ **Altamente Configurável:**  Configure o provedor de IA, chave de API, modelo e outras opções.

## 🚀 Instalação

### Via pipx (Recomendado)

`pipx` é uma ferramenta que instala e executa aplicativos Python em ambientes isolados, garantindo que as dependências do Seshat não interfiram em outros projetos.

```bash
# 1. Instalar pipx (se você ainda não tiver)
python3 -m pip install --user pipx
python3 -m pipx ensurepath

# 2. Instalar Seshat
pipx install git+[https://github.com/juniormartinxo/seshat.git](https://github.com/juniormartinxo/seshat.git)
````

### Instalação para Desenvolvimento

Para contribuir com o desenvolvimento do Seshat, siga estas etapas:

```bash
# 1. Clonar o repositório
git clone [https://github.com/juniormartinxo/seshat.git](https://github.com/juniormartinxo/seshat.git)
cd seshat

# 2. Criar um ambiente virtual (altamente recomendado)
python3 -m venv .venv
source .venv/bin/activate  # No Windows: .venv\Scripts\activate

# 3. Instalar as dependências
pip install -e .
```

## ⚙️ Configuração

Seshat suporta os seguintes provedores de IA:

  * **DeepSeek API:**  Um provedor de IA online.
  * **Claude API (Anthropic):** Outro provedor de IA online.
  * **Ollama (Local):**  Execute modelos de IA localmente usando Ollama.

### Configuração Rápida (DeepSeek/Claude)

1.  **Obtenha sua Chave de API:**

      * **DeepSeek:**  [Link para a documentação do DeepSeek](https://platform.deepseek.com/docs)
      * **Claude:** [Link para a documentação do Claude](https://console.anthropic.com/dashboard)

2.  **Configure via CLI:**

    ```bash
    seshat config --provider deepseek  # Ou claude
    seshat config --api-key SUA_CHAVE_API
    seshat config --model seu-modelo #ex: deepseek-coder-v2, claude-3-haiku-20240307
    ```

    Ou, alternativamente defina as variáveis de ambiente em um arquivo `.env`:
    ` bash AI_PROVIDER=deepseek|claude|ollama API_KEY=sua_chave_aqui AI_MODEL=seu-modelo  `

### Configuração do Ollama (IA Local)

1.  **Instale o Ollama:** Siga as instruções de instalação em [https://ollama.ai](https://ollama.ai).
2.  **Inicie o Servidor Ollama:**
    ```bash
    ollama serve
    ```
3.  **Baixe um Modelo Compatível:**  Por exemplo, o `deepseek-coder`:
    ```bash
    ollama pull deepseek-coder
    ```
    (Você pode encontrar outros modelos em [https://ollama.ai/library](https://www.google.com/url?sa=E&source=gmail&q=https://ollama.ai/library))
4.  **Configure o Seshat**
    ```bash
     seshat config --provider ollama
    ```

## 💻 Uso

**Exemplo Básico:**

```bash
git add .
seshat commit
```

**Exemplos Avançados:**

  * Commit com escopo e confirmação automática:

    ```bash
    git add src/
    seshat commit --scope core --yes
    ```

  * Commit do tipo "feat" com breaking change:

    ```bash
    git add .
    seshat commit --type feat --breaking "Esta mudança quebra a compatibilidade da API."
    ```

  * Especificando o provedor e modelo (sobrescreve a configuração):

    ```bash
    seshat commit --provider claude --model claude-3-haiku-20240307 --verbose
    ```

      * `--yes`: Confirma a mensagem de commit gerada automaticamente, sem interação.
      * `--verbose`: Exibe informações detalhadas sobre o processo.
      * `--type`: Força a utilização de um tipo de commit.
      * `--scope`: Adiciona um escopo (contexto) ao commit.
      * `--breaking`: Adiciona uma descrição para um *breaking change*.

## 📚 Tipos de Commit (Conventional Commits)

| Tipo       | Descrição                                                                 |
| :--------- | :------------------------------------------------------------------------ |
| `feat`     | Adiciona uma nova funcionalidade.                                         |
| `fix`      | Corrige um bug.                                                           |
| `docs`     | Altera a documentação.                                                   |
| `style`    | Realiza mudanças de formatação (sem impacto no código).                   |
| `refactor` | Refatora o código (sem adicionar funcionalidades ou corrigir bugs).         |
| `perf`     | Melhora o desempenho.                                                     |
| `test`     | Adiciona ou corrige testes.                                                |
| `chore`    | Tarefas de manutenção (e.g., atualizar dependências).                      |
| `build`    | Mudanças relacionadas ao sistema de build.                                 |
| `ci`       | Mudanças relacionadas à integração contínua (CI).                       |
| `revert`   | Reverte um commit anterior.                                                |

## ⚠️ Solução de Problemas

**Problemas de Configuração:**

```bash
# Verificar a configuração atual
seshat config

# Redefinir a configuração
seshat config --provider deepseek  # Ou outro provedor
seshat config --api-key SUA_NOVA_CHAVE
```

**Problemas com o Ollama:**

```bash
# Verificar se o servidor Ollama está rodando
curl http://localhost:11434/api/version

# Listar os modelos instalados
ollama list

# Problemas de conexão com a API? Verifique sua internet e a chave de API.
```

**Diff Muito Grande:**

Se o `git diff` for muito grande (acima de 2500 caracteres), o Seshat irá avisá-lo.  Considere dividir suas alterações em commits menores:

```bash
git add -p  # Adiciona as mudanças interativamente, em pedaços
```

**Erros de Autenticação:**

  * Verifique se sua chave de API está correta e não expirou.
  * Verifique se você tem permissão para usar o modelo especificado.

## 🤝 Contribuindo

Contribuições são bem-vindas\!  Se você encontrar um bug, tiver uma sugestão ou quiser adicionar uma nova funcionalidade:

1.  Faça um fork do repositório.
2.  Crie um branch para sua feature: `git checkout -b minha-nova-feature`
3.  Faça commit das suas mudanças: `seshat commit` (use a própria ferramenta\!)
4.  Faça push para o branch: `git push origin minha-nova-feature`
5.  Abra um Pull Request.

🐛 [Reportar Bug](https://github.com/juniormartinxo/seshat/issues)

✨ [Sugerir Feature](https://github.com/juniormartinxo/seshat/issues)

## 📝 Licença

MIT © [Junior Martins](https://github.com/juniormartinxo)
