# Seshat 🤖

![Python](https://img.shields.io/badge/Python-3.10%2B-blue)
[![Seshat CI](https://github.com/juniormartinxo/seshat/actions/workflows/main.yml/badge.svg)](https://github.com/juniormartinxo/seshat/actions/workflows/main.yml)
![Git](https://img.shields.io/badge/Git-Integrado-green)
![License](https://img.shields.io/badge/License-MIT-orange)
[![SLSA 3](https://slsa.dev/images/gh-badge-level3.svg)](https://slsa.dev)

Uma CLI poderosa para automatizar a criação de mensagens de commit seguindo o padrão Conventional Commits, utilizando o poder da Inteligência Artificial.

## ✨ Recursos

* ✅ **Múltiplos Provedores de IA:** Suporte para DeepSeek API, Claude API (Anthropic), OpenAI API, Gemini API (Google) e Ollama (local).
* 📏 **Validação de Tamanho do Diff:**  Alertas para diffs grandes, com limites configuráveis.
* 🔍 **Verificação de Arquivos Staged:** Garante que você não se esqueça de adicionar arquivos ao commit.
* 📝 **Conventional Commits com Validação:** Gera mensagens seguindo o padrão e bloqueia commits com mensagem vazia ou inválida.
* 🤝 **Confirmação Interativa:**  Permite revisar e editar a mensagem de commit gerada pela IA antes de confirmar.
* ⚙️ **Altamente Configurável:**  Configure o provedor de IA, chave de API, modelo e outras opções.
* 📅 **Data de Commit Personalizada:** Defina datas específicas para seus commits.
* 🔄 **Fluxo de Commits em Lote:** Processe múltiplos arquivos, gerando um commit individual para cada um.
* 🧹 **Saída de Terminal Profissional:** UI consistente, progresso em tempo real e saída do Git silenciosa por padrão (use `--verbose` para detalhes).

## 🚀 Instalação

### Via pipx (Recomendado)

`pipx` é uma ferramenta que instala e executa aplicativos Python em ambientes isolados, garantindo que as dependências do Seshat não interfiram em outros projetos.

```bash
# 1. Instalar pipx (se você ainda não tiver)
# Debian/Ubuntu (PEP 668): prefira o pacote do sistema
sudo apt update
sudo apt install pipx
pipx ensurepath

# Outras distros
python3 -m pip install --user pipx
python3 -m pipx ensurepath

# 2. Instalar Seshat
pipx install git+https://github.com/juniormartinxo/seshat.git
```

### Instalação para Desenvolvimento

Para contribuir com o desenvolvimento do Seshat, siga estas etapas:

```bash
# 1. Clonar o repositório
git clone https://github.com/juniormartinxo/seshat.git
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
* **OpenAI API:** Provedor de IA online, muito conhecido como ChatGPT.
* **Gemini API (Google):** Provedor de IA do Google.
* **Ollama (Local):**  Execute modelos de IA localmente usando Ollama.

### Configuração Rápida (Provedores Online)

1. **Obtenha sua Chave de API:**

      * **DeepSeek:**  [Link para a documentação do DeepSeek](https://platform.deepseek.com/docs)
      * **Claude:** [Link para a documentação do Claude](https://console.anthropic.com/dashboard)
      * **OpenAI:** [Link para a documentação do OpenAI](https://platform.openai.com/)
      * **Gemini:** [Link para a documentação do Gemini](https://ai.google.dev/gemini-api/docs/quickstart)

2. **Configure via CLI:**

    ```bash
    seshat config --provider SEU_PROVIDER # Provedores aceitos deepseek|claude|ollama|openai|gemini
    seshat config --api-key SUA_CHAVE_API
    seshat config --model SEU_MODEL #ex: deepseek-coder-v2, claude-3-haiku-20240307, gemini-2.5-flash
    ```

    Ou, alternativamente defina as variáveis de ambiente em um arquivo `.env`:

    ```bash
    AI_PROVIDER=deepseek|claude|ollama|openai|gemini 
    API_KEY=sua_chave_aqui 
    AI_MODEL=seu-modelo
    ```

### Configuração do Ollama (IA Local)

1. **Instale o Ollama:** Siga as instruções de instalação em [https://ollama.ai](https://ollama.ai).

2. **Inicie o Servidor Ollama:**

    ```bash
    ollama serve
    ```

3. **Baixe um Modelo Compatível:** Por exemplo, o `deepseek-coder`:

  ```bash
  ollama pull deepseek-coder
  ```

(Você pode encontrar outros modelos em [https://ollama.ai/library](https://ollama.ai/library))

1. **Configure o Seshat**

    ```bash
    seshat config --provider ollama
    ```

### Configuração dos Limites de Diff

Você pode configurar os limites para o tamanho do diff:

```bash
# Configurar limite máximo (padrão: 3000 caracteres)
seshat config --max-diff 5000

# Configurar limite de aviso (padrão: 2500 caracteres)
seshat config --warn-diff 4000
```

### Configuração da Linguagem dos Commits

Escolha o idioma das mensagens geradas pela IA (também afeta alertas da CLI):

```bash
# PT-BR (padrão), ENG, ESP, FRA, DEU, ITA
seshat config --language PT-BR
seshat config --language ENG
```

Ou via `.env`:

```bash
COMMIT_LANGUAGE=PT-BR|ENG|ESP|FRA|DEU|ITA
```

## 💻 Uso

### Commit Básico

```bash
git add .
seshat commit
```

Por padrão, o Seshat executa o `git commit` em modo silencioso para manter a saída limpa.  
Use `--verbose` para ver o diff analisado e a saída completa do Git.

### Commits com Data Personalizada

```bash
# Commit com data específica
seshat commit --date="2025-02-20 14:30:00"

# Usar descrições relativas
seshat commit --date="yesterday"
seshat commit --date="1 week ago"
seshat commit --date="last Friday 17:00"
```

### Fluxo de Commits em Lote

Processe e comite múltiplos arquivos individualmente:

```bash
# Processar os primeiros 5 arquivos modificados
seshat flow 5

# Processar todos os arquivos modificados
seshat flow

# Processar os 3 primeiros arquivos sem confirmação
seshat flow 3 --yes

# Processar arquivos em um diretório específico
seshat flow 10 --path ./src
```

Notas importantes sobre o fluxo:

* Cada arquivo é processado de forma isolada (o commit contém apenas aquele arquivo).
* Em execuções concorrentes, o Seshat usa um lock por arquivo. Se outro agente já estiver processando o arquivo, ele será **pulado** para evitar bloqueios e gastos desnecessários com IA.
* O resumo final mostra contagem de **Sucesso**, **Falhas** e **Pulados**.

### Exemplos Avançados

* Commit com confirmação automática e limite de diff personalizado:

    ```bash
    git add src/
    seshat commit --yes --max-diff 10000
    ```

* Commit com provedor específico e data:

    ```bash
    seshat commit --provider claude --date="yesterday 14:00" --verbose
    ```

* Fluxo de commits com data específica:

    ```bash
    seshat flow 5 --date="2025-02-20" --yes
    ```

### Opções Disponíveis

* **Comando `commit`**:
  * `--yes` ou `-y`: Pula todas as confirmações.
  * `--verbose` ou `-v`: Exibe diff analisado e saída do Git.
  * `--date` ou `-d`: Define a data do commit.
  * `--max-diff`: Sobrescreve o limite máximo do diff para este commit.
  * `--provider`: Especifica o provedor de IA.
  * `--model`: Especifica o modelo de IA.

* **Comando `flow`**:
  * Todas as opções do comando `commit` mais:
  * `--path` ou `-p`: Caminho para buscar arquivos modificados.
  * `COUNT`: Número máximo de arquivos a processar (argumento posicional).

* **Comando `config`**:
  * `--api-key`: Configura a chave de API.
  * `--provider`: Configura o provedor padrão.
  * `--model`: Configura o modelo padrão.
  * `--max-diff`: Configura o limite máximo do diff.
  * `--warn-diff`: Configura o limite de aviso do diff.
  * `--language`: Configura a linguagem das mensagens (PT-BR, ENG, ESP, FRA, DEU, ITA).

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
seshat config --provider SEU_PROVIDER # Provedores aceitos deepseek|claude|ollama|openai|gemini
seshat config --api-key SUA_NOVA_CHAVE
seshat config --model MODELO_DO_SEU_PROVIDER #ex: deepseek-coder-v2, claude-3-haiku-20240307, gemini-2.5-flash
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

Se o diff for muito grande (acima do limite configurado), o Seshat irá avisá-lo. Você pode:

```bash
# Aumentar o limite para este commit
seshat commit --max-diff 10000

# Aumentar o limite global
seshat config --max-diff 10000

# Ou dividir suas alterações em commits menores
git add -p  # Adiciona as mudanças interativamente, em pedaços
```

**Mensagem de Commit Vazia ou Inválida:**

Se a IA retornar uma mensagem vazia ou fora do padrão Conventional Commits, o Seshat aborta antes do Git.
Tente:

1. Rodar novamente o comando (`seshat commit`/`seshat flow`).
2. Reduzir ou organizar o diff (commits menores ajudam).
3. Fazer o commit manualmente, se necessário.

**Erros de Autenticação:**

* Verifique se sua chave de API está correta e não expirou.
* Verifique se você tem permissão para usar o modelo especificado.

**Problemas com o Comando Flow:**

Se o comando `flow` não for reconhecido, verifique se a instalação está atualizada:

```bash
pip install --upgrade git+https://github.com/juniormartinxo/seshat.git
```

**Flow concorrente e arquivos pulados:**

Quando múltiplos agentes/execuções rodam ao mesmo tempo, arquivos em processamento por outro agente serão pulados automaticamente. Isso evita commits cruzados e reduz custos com IA.

## 🤝 Contribuindo

Contribuições são bem-vindas! Se você encontrar um bug, tiver uma sugestão ou quiser adicionar uma nova funcionalidade:

1. Faça um fork do repositório.
2. Crie um branch para sua feature: `git checkout -b minha-nova-feature`
3. Faça commit das suas mudanças: `seshat commit` (use a própria ferramenta!)
4. Faça push para o branch: `git push origin minha-nova-feature`
5. Abra um Pull Request.

🐛 [Reportar Bug](https://github.com/juniormartinxo/seshat/issues)

✨ [Sugerir Feature](https://github.com/juniormartinxo/seshat/issues)

## 📝 Licença

MIT © [Junior Martins](https://github.com/juniormartinxo)
