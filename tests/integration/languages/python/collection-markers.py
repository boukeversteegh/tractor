# Python collection literals unify by produced type. <list>, <dict>, <set>
# carry exhaustive <literal/> or <comprehension/> markers so queries can
# distinguish `[x for x in xs]` from `[1, 2, 3]` without kind-specific
# element names like list_comprehension vs list.

nums = [1, 2, 3]                        # list + <literal/>
squares = [x * x for x in nums]         # list + <comprehension/>

pairs = {"a": 1, "b": 2}                # dict + <literal/>
inverted = {v: k for k, v in pairs.items()}  # dict + <comprehension/>

unique = {1, 2, 3}                      # set + <literal/>
uniq_sq = {x * x for x in nums}         # set + <comprehension/>

gen = (x for x in nums)                 # generator + <comprehension/>
