"""Detect training-serving skew in ML pipelines."""

from pathlib import Path
from typing import List, Dict, Any
from ..base import Analyzer, Finding, Severity


class TrainServeSkewAnalyzer(Analyzer):
    """
    Detects differences between training and serving code.

    Checks for:
    - Different preprocessing in training vs serving
    - Different feature computation
    - Different normalization approaches
    """

    def name(self) -> str:
        return "ML Train-Serve Skew Detector"

    def file_patterns(self) -> List[str]:
        return ["**/*.py"]

    def analyze(
        self,
        graph: Dict[str, Any],
        changed_files: List[Dict[str, Any]],
    ) -> List[Finding]:
        """Detect training-serving skew."""
        findings = []

        # TODO: Implement train-serve skew detection
        # Compare feature computation in:
        # - Training directories (training/, train/, ml/train/)
        # - Serving directories (serving/, inference/, api/)

        return findings
