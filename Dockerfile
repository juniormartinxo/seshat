FROM python:3.11-slim

WORKDIR /app

# Install git as it is often required for pip install
RUN apt-get update && apt-get install -y git && rm -rf /var/lib/apt/lists/*

# Install build tools and common CI tools to cache them
RUN pip install --upgrade pip
RUN pip install ruff mypy pytest types-requests

# Copy files only to install dependencies first if we wanted to optimize layering,
# but for a dev container where we mount volume, we just ensure tools are there.
# We will run pip install -e . at runtime or here.
# Let's install current deps here to enable faster startups, but allow overriding.
COPY . .
RUN pip install -e .

CMD ["bash"]
