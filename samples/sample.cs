public static class QueryHelpers
{
    // This could be an extension method but isn't
    public static List<T> Execute<T>(IQueryable<T> source)
    {
        return source.ToList();
    }

    // This IS an extension method
    public static IQueryable<T> Where<T>(this IQueryable<T> source)
    {
        return source;
    }
}
