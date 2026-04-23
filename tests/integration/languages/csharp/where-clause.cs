// C# `where` clause constraints attach to the matching <generic>
// element. Shape constraints (class / struct / notnull / unmanaged /
// new) become empty markers that compose; type bounds wrap in
// <extends><type>…</type></extends>.
//
// Queries:
//   //generic[new]                 -> generics that require a default ctor
//   //generic[class]               -> generics constrained to reference types
//   //generic[extends/type[name='IComparable']]
//                                  -> generics constrained to IComparable
//   //generic[name='T'][extends]   -> T with any type bound

using System;

class Repo<T, U, V>
    where T : class, IComparable<T>, new()
    where U : struct
    where V : notnull
{
}
