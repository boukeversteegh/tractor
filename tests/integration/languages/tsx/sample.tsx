// Simple TSX example
import React, { useState } from 'react';

interface Props {
    name: string;
    age?: number;
}

function Greeting({ name, age }: Props) {
    const [count, setCount] = useState(0);

    return (
        <div className="greeting">
            <h1>Hello, {name}!</h1>
            {age && <span>Age: {age}</span>}
            <button onClick={() => setCount(count + 1)}>
                Clicked {count} times
            </button>
        </div>
    );
}

export default Greeting;
