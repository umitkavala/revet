"""Setup script for revet-analyzers package."""

from setuptools import setup, find_packages

setup(
    name="revet-analyzers",
    version="0.1.0",
    description="Domain-specific analyzers for Revet code review agent",
    author="Umit Kavala",
    packages=find_packages(),
    python_requires=">=3.8",
    install_requires=[
        # Add dependencies as needed
    ],
    extras_require={
        "dev": [
            "pytest>=7.0.0",
            "pytest-cov>=4.0.0",
            "black>=23.0.0",
            "ruff>=0.1.0",
        ],
    },
)
