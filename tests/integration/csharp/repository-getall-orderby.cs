public class UserRepository
{
    public List<User> GetAll() { return _context.Users.ToList(); }  // MATCH - missing OrderBy
    public List<User> GetAllOrdered() { return _context.Users.OrderBy(u => u.Name).ToList(); }  // OK
}

public class MockUserRepository
{
    public List<User> GetAll() { return mock.ToList(); }  // Skip - Mock class
}
