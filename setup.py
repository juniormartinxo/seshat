from pathlib import Path
import re
from setuptools import setup, find_packages

ROOT = Path(__file__).parent

def read_version() -> str:
    version_path = ROOT / "seshat" / "__init__.py"
    content = version_path.read_text(encoding="utf-8")
    match = re.search(r'__version__\s*=\s*"([^"]+)"', content)
    if not match:
        raise RuntimeError("Unable to find __version__ in seshat/__init__.py")
    return match.group(1)

setup(
    name="seshat",
    version=read_version(),
    packages=find_packages(),
    install_requires=[
        "click>=8.0",
        "rich>=13.0",
        "typer>=0.12.0",
        "requests>=2.26",
        "python-dotenv>=0.19",
        "anthropic>=0.19",
        "openai==1.65.1",
        "google-genai",
        "keyring>=24.0",
    ],
    python_requires=">=3.8",
    entry_points={
        "console_scripts": ["seshat=seshat.cli:cli"],
    },
    author="Junior Martins",
    author_email="amjr.box@gmail.com",
    description="CLI para commits automatizados usando Conventional Commits e DeepSeek API",
    long_description=(ROOT / "README.md").read_text(encoding="utf-8"),
    long_description_content_type="text/markdown",
    keywords="git commit conventional-commits ai deepseek",
    url="https://github.com/juniormartinxo/seshat",
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "Topic :: Software Development :: Version Control :: Git",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
    ],
)
