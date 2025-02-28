from setuptools import setup, find_packages

setup(
    name="seshat",
    version="0.1.4",  # Versão incrementada por causa da correção
    packages=find_packages(),
    install_requires=[
        "click>=8.0",
        "requests>=2.26",
        "python-dotenv>=0.19",
        "anthropic>=0.19",
        "setuptools>=75.8.0",
        "openai==1.65.1"
    ],
    python_requires=">=3.8",  # Especifica versão mínima do Python
    entry_points={
        "console_scripts": ["seshat=seshat.cli:cli"],
    },
    author="Junior Martins",
    author_email="amjr.box@gmail.com",
    description="CLI para commits automatizados usando Conventional Commits e DeepSeek API",
    long_description=open("README.md").read(),
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