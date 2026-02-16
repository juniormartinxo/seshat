from unittest.mock import MagicMock
import pytest
from seshat import ui


@pytest.fixture
def mock_rich_console(monkeypatch):
    # Mockando a classe 'rich.console.Console' e sua instância
    mock_console_instance = MagicMock()
    mock_console_class = MagicMock(return_value=mock_console_instance)
    monkeypatch.setattr(ui, "Console", mock_console_class)
    # Precisamos mockar o size também para o hr
    mock_console_instance.size.width = 100
    return mock_console_class, mock_console_instance


# Testando helper checks
def test_is_tty(monkeypatch):
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    ui.set_force_rich(None)
    assert ui.is_tty() is True

    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    ui.set_force_rich(None)
    assert ui.is_tty() is False


# Testando echo
def test_echo_standard(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    # Testando com isatty=True
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    ui.echo("Hello")

    mock_console_class.assert_called_with(
        stderr=False,
        color_system="auto",
        force_terminal=True,
    )
    mock_console_instance.print.assert_called_with("Hello")


def test_echo_stderr(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    ui.set_force_rich(None)
    ui._CONSOLE_ERR = None

    ui.echo("Error", err=True)

    mock_console_class.assert_called_with(
        stderr=True,
        color_system="auto",
        force_terminal=True,
    )
    mock_console_instance.print.assert_called_with("Error")


# Testando hr (Horizontal Rule)
def test_hr_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    ui.hr()

    assert mock_console_instance.print.call_count == 1


def test_hr_no_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    ui.hr()

    mock_console_instance.print.assert_called_with("─" * 80)


# Testando title
def test_title_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    monkeypatch.setattr(ui, "Panel", MagicMock())
    ui.set_force_rich(None)
    ui._CONSOLE = None

    ui.title("Test Title")

    ui.Panel.assert_called_once()
    assert mock_console_instance.print.call_count == 2


def test_title_no_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    ui.title("Test Title")

    assert mock_console_instance.print.call_count == 3


# Testando section
def test_section_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    ui.section("Section")

    assert mock_console_instance.print.call_count == 2


def test_section_no_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    ui.section("Section")

    mock_console_instance.print.assert_called_with("\nSection")


# Testando confirm
def test_confirm(monkeypatch):
    # Ensure isatty is False to use typer.confirm
    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    mock_confirm = MagicMock(return_value=True)
    monkeypatch.setattr("typer.confirm", mock_confirm)

    result = ui.confirm("Are you sure?")

    mock_confirm.assert_called_with("Are you sure?", default=False)
    assert result is True


# Testando prompt
def test_prompt(monkeypatch):
    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    mock_prompt = MagicMock(return_value="user_input")
    monkeypatch.setattr("typer.prompt", mock_prompt)

    result = ui.prompt("Enter value")

    mock_prompt.assert_called_with("Enter value", default=None, show_default=True, type=None)
    assert result == "user_input"


# Testando prompt with choices - Non-Rich (Typer)
def test_prompt_choices_no_rich(monkeypatch):
    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    mock_prompt = MagicMock(return_value="choice1")
    click_choice_mock = MagicMock()
    monkeypatch.setattr("typer.prompt", mock_prompt)
    monkeypatch.setattr("click.Choice", click_choice_mock)

    ui.prompt("Select", choices=["a", "b"])

    click_choice_mock.assert_called_with(["a", "b"])
    mock_prompt.assert_called()


# Testando prompt with choices - Rich
def test_prompt_choices_rich(monkeypatch):
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    # We must patch seshat.ui.Prompt because it's already imported in ui.py
    mock_ask = MagicMock(return_value="choice1")
    monkeypatch.setattr(ui.Prompt, "ask", mock_ask)

    result = ui.prompt("Select", choices=["a", "b"])

    mock_ask.assert_called()
    assert result == "choice1"


# Testando Status Context Manager
def test_status_context(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    mock_status_obj = MagicMock()
    mock_console_instance.status.return_value = mock_status_obj

    with ui.status("Working..."):
        pass

    assert mock_console_instance.status.called
    mock_status_obj.__enter__.assert_called()
    mock_status_obj.__exit__.assert_called()


def test_status_update(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    mock_status_obj = MagicMock()
    mock_console_instance.status.return_value = mock_status_obj

    s = ui.status("Start")
    with s:
        s.update("New message")

    assert mock_status_obj.update.called


# Testando Table
def test_table_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: True)
    mock_table_class = MagicMock()
    monkeypatch.setattr(ui, "Table", mock_table_class)
    monkeypatch.setattr(ui, "Padding", lambda *args, **kwargs: "PAD")
    ui.set_force_rich(None)
    ui._CONSOLE = None

    columns = ["Col1", "Col2"]
    rows = [["Val1", "Val2"]]

    ui.table("Title", columns, rows)

    from unittest.mock import ANY
    mock_table_class.assert_called_with(
        title="Title",
        title_style=ANY,
        box=ANY,
        border_style=ANY,
        header_style=ANY,
        show_header=True,
        padding=(0, 1),
        expand=False,
    )
    mock_console_instance.print.assert_called()


def test_table_no_rich(mock_rich_console, monkeypatch):
    mock_console_class, mock_console_instance = mock_rich_console
    monkeypatch.setattr("sys.stdout.isatty", lambda: False)
    ui.set_force_rich(None)
    ui._CONSOLE = None

    columns = ["Col1", "Col2"]
    rows = [["Val1", "Val2"]]

    ui.table("Title", columns, rows)

    assert mock_console_instance.print.call_count == 2
