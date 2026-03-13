public class UserService
{
    // GetUser: missing AsNoTracking - should be flagged
    public UserDto GetUser(int id)
    {
        var user = _context.Users.First(u => u.Id == id);
        return Map(user);
    }

    // GetUserTracked: has AsNoTracking - OK
    public UserDto GetUserTracked(int id)
    {
        var user = _context.Users.AsNoTracking().First(u => u.Id == id);
        return Map(user);
    }
}
