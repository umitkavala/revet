"""Base classes for domain analyzers."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import List, Optional, Dict, Any


class Severity(Enum):
    """Finding severity levels."""

    ERROR = "error"
    WARNING = "warning"
    INFO = "info"


@dataclass
class Evidence:
    """Supporting evidence for a finding."""

    description: str
    file: Path
    line: int
    snippet: str


@dataclass
class Finding:
    """A code issue discovered by an analyzer."""

    id: str
    severity: Severity
    message: str
    file: Path
    line: int
    end_line: Optional[int] = None
    evidence: List[Evidence] = field(default_factory=list)
    fixable: bool = False
    deterministic: bool = True  # True = no LLM involved

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "id": self.id,
            "severity": self.severity.value,
            "message": self.message,
            "file": str(self.file),
            "line": self.line,
            "end_line": self.end_line,
            "evidence": [
                {
                    "description": e.description,
                    "file": str(e.file),
                    "line": e.line,
                    "snippet": e.snippet,
                }
                for e in self.evidence
            ],
            "fixable": self.fixable,
            "deterministic": self.deterministic,
        }


class Analyzer(ABC):
    """Base class for domain-specific analyzers."""

    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """Initialize the analyzer with optional configuration."""
        self.config = config or {}

    @abstractmethod
    def name(self) -> str:
        """Return the human-readable name of this analyzer."""
        pass

    @abstractmethod
    def file_patterns(self) -> List[str]:
        """Return glob patterns for files this analyzer cares about."""
        pass

    @abstractmethod
    def analyze(
        self,
        graph: Dict[str, Any],
        changed_files: List[Dict[str, Any]],
    ) -> List[Finding]:
        """
        Run analysis on the code graph and changed files.

        Args:
            graph: JSON representation of the code graph
            changed_files: List of changed file information

        Returns:
            List of findings discovered
        """
        pass
