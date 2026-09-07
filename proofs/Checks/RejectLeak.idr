module Checks.RejectLeak

import Cleave.Kernel.HandleLinearity

%default total

-- Expected failure: a linearly bound variable cannot be discarded implicitly.
bad : (1 h : Handle) -> Perp
bad _ = AtPerp
