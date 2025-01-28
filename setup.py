from setuptools import setup, find_packages

setup(
    name="seshat",
    version="0.1.0",
    packages=find_packages(),
    install_requires=[
        'click>=8.0',
        'requests>=2.26',
        'python-dotenv>=0.19',
        'anthropic>=0.19'
    ],
    entry_points={
        'console_scripts': [
            'seshat=seshat.cli:cli'
        ],
    },
    author="Junior Martins <amjr.box@gmail.com>",
    description="CLI para commits automatizados usando Conventional Commits e DeepSeek API",
    keywords="git commit conventional-commits ai deepseek",
    url="https://github.com/juniormartinxo/seshat",
)