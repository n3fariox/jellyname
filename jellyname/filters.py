from dataclasses import dataclass

@dataclass
class Filters:
    lang: str = None


DefaultFilters = Filters()
