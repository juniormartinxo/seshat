from unittest.mock import MagicMock
import pytest
from seshat import ui

@pytest.fixture
def mock_rich_console(mocker):
    # Mockando a classe 'rich.console.Console' e sua instância
    mock_console_instance = MagicMock()
    mock_console_class = mocker.patch("seshat.ui.Console", return_value=mock_console_instance)
    # Precisamos mockar o size também para o hr
    mock_console_instance.size.width = 100
    return mock_console_class, mock_console_instance

# Testando helper checks
def test_is_tty(mocker):
    mocker.patch("sys.stdout.isatty", return_value=True)
    assert ui.is_tty() is True
    
    mocker.patch("sys.stdout.isatty", return_value=False)
    assert ui.is_tty() is False

# Testando echo
def test_echo_standard(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    # Testando com isatty=True
    mocker.patch("sys.stdout.isatty", return_value=True)
    
    ui.echo("Hello")
    
    mock_console_class.assert_called_with(stderr=False, color_system="auto")
    mock_console_instance.print.assert_called_with("Hello")

def test_echo_stderr(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=True)
    
    ui.echo("Error", err=True)
    
    mock_console_class.assert_called_with(stderr=True, color_system="auto")
    mock_console_instance.print.assert_called_with("Error")

# Testando hr (Horizontal Rule)
def test_hr_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=True)
    
    ui.hr()
    
    # Implementation caps width at 80
    mock_console_instance.print.assert_called_with("─" * 80, style="bright_black")

def test_hr_no_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=False)
    
    ui.hr()
    
    mock_console_instance.print.assert_called_with("─" * 80)


# Testando title
def test_title_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=True)
    mock_panel = mocker.patch("seshat.ui.Panel")
    
    ui.title("Test Title")
    
    mock_panel.assert_called_once()
    mock_console_instance.print.assert_called()

def test_title_no_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=False)
    
    ui.title("Test Title")
    
    assert mock_console_instance.print.call_count == 3

# Testando section
def test_section_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=True)
    
    ui.section("Section")
    
    mock_console_instance.print.assert_called_with("\nSection", style="cyan bold")

def test_section_no_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=False)

    ui.section("Section")
    
    mock_console_instance.print.assert_called_with("\nSection")

# Testando confirm
def test_confirm(mocker):
    # Ensure isatty is False to use typer.confirm
    mocker.patch("sys.stdout.isatty", return_value=False)
    mock_confirm = mocker.patch("typer.confirm", return_value=True)
    
    result = ui.confirm("Are you sure?")
    
    mock_confirm.assert_called_with("Are you sure?", default=False)
    assert result is True

# Testando prompt
def test_prompt(mocker):
    mocker.patch("sys.stdout.isatty", return_value=False)
    mock_prompt = mocker.patch("typer.prompt", return_value="user_input")
    
    result = ui.prompt("Enter value")
    
    mock_prompt.assert_called_with("Enter value", default=None, show_default=True, type=None)
    assert result == "user_input"

# Testando prompt with choices - Non-Rich (Typer)
def test_prompt_choices_no_rich(mocker):
    mocker.patch("sys.stdout.isatty", return_value=False)
    mock_prompt = mocker.patch("typer.prompt", return_value="choice1")
    click_choice_mock = mocker.patch("click.Choice")
    
    ui.prompt("Select", choices=["a", "b"])
    
    click_choice_mock.assert_called_with(["a", "b"])
    mock_prompt.assert_called()

# Testando prompt with choices - Rich
def test_prompt_choices_rich(mocker):
    mocker.patch("sys.stdout.isatty", return_value=True)
    # We must patch seshat.ui.Prompt because it's already imported in ui.py
    mock_ask = mocker.patch("seshat.ui.Prompt.ask", return_value="choice1")
    
    result = ui.prompt("Select", choices=["a", "b"])
    
    mock_ask.assert_called()
    assert result == "choice1"

# Testando Status Context Manager
def test_status_context(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=True)
    
    mock_status_obj = MagicMock()
    mock_console_instance.status.return_value = mock_status_obj
    
    with ui.status("Working..."):
        pass
        
    mock_console_instance.status.assert_called_with("Working...")
    mock_status_obj.__enter__.assert_called()
    mock_status_obj.__exit__.assert_called()

def test_status_update(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=True)
    
    mock_status_obj = MagicMock()
    mock_console_instance.status.return_value = mock_status_obj
    
    s = ui.status("Start")
    with s:
        s.update("New message")
        
    mock_status_obj.update.assert_called_with("New message")

# Testando Table
def test_table_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=True)
    mock_table_class = mocker.patch("seshat.ui.Table")
    
    columns = ["Col1", "Col2"]
    rows = [["Val1", "Val2"]]
    
    ui.table("Title", columns, rows)
    
    mock_table_class.assert_called_with(title="Title", box=mocker.ANY, show_header=True)
    mock_console_instance.print.assert_called()

def test_table_no_rich(mock_rich_console, mocker):
    mock_console_class, mock_console_instance = mock_rich_console
    mocker.patch("sys.stdout.isatty", return_value=False)
    
    columns = ["Col1", "Col2"]
    rows = [["Val1", "Val2"]]
    
    ui.table("Title", columns, rows)
    
    assert mock_console_instance.print.call_count == 2
