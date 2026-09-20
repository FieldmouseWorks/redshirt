"""External experiment runner. Adapters retain domain rules and evaluation."""
from .runner import Candidate, Limits, Observation, Stop, Verdict, run

__all__ = ["Candidate", "Limits", "Observation", "Stop", "Verdict", "run"]
