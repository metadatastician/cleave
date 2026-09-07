module Checks.RejectDuplication

import Cleave.Kernel.HandleLinearity

%default total

-- Expected failure: the same linearly bound variable cannot be consumed twice.
bad : (1 h : Handle) -> (Perp, Perp)
bad h = (teardown h, teardown h)
