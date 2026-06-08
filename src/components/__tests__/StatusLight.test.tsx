import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import App from '../../App';
import { StatusLabel } from '../StatusLabel';
import { StatusLight } from '../StatusLight';

afterEach(() => {
  cleanup();
});

describe('StatusLight', () => {
  it('renders a vertical three-lens traffic-light shell', () => {
    const { container } = render(<StatusLight status="running" />);

    expect(screen.getByTestId('status-light')).toHaveAttribute('data-status', 'running');
    expect(screen.getByTestId('lens-red')).toBeInTheDocument();
    expect(screen.getByTestId('lens-yellow')).toBeInTheDocument();
    expect(screen.getByTestId('lens-green')).toBeInTheDocument();
    expect(container.querySelector('.status-light__top')).toBeNull();
    expect(container.querySelector('.status-light__foot')).toBeNull();
  });

  it('activates the yellow lens for running', () => {
    const { container } = render(<StatusLight status="running" />);
    const shell = within(container).getByTestId('status-light');
    const query = within(shell);

    expect(query.getByTestId('lens-red')).toHaveAttribute('data-active', 'false');
    expect(query.getByTestId('lens-yellow')).toHaveAttribute('data-active', 'true');
    expect(query.getByTestId('lens-green')).toHaveAttribute('data-active', 'false');
  });

  it('activates the red lens for pending user input', () => {
    const { container } = render(<StatusLight status="pending_user" />);
    const shell = within(container).getByTestId('status-light');
    const query = within(shell);

    expect(query.getByTestId('lens-red')).toHaveAttribute('data-active', 'true');
    expect(query.getByTestId('lens-yellow')).toHaveAttribute('data-active', 'false');
    expect(query.getByTestId('lens-green')).toHaveAttribute('data-active', 'false');
  });

  it('activates the green lens for done', () => {
    const { container } = render(<StatusLight status="done" />);
    const shell = within(container).getByTestId('status-light');
    const query = within(shell);

    expect(query.getByTestId('lens-red')).toHaveAttribute('data-active', 'false');
    expect(query.getByTestId('lens-yellow')).toHaveAttribute('data-active', 'false');
    expect(query.getByTestId('lens-green')).toHaveAttribute('data-active', 'true');
  });
});

describe('StatusLabel', () => {
  it('renders status text with spaces', () => {
    render(<StatusLabel status="pending_user" />);

    expect(screen.getByText('PENDING USER')).toBeInTheDocument();
  });
});

describe('App', () => {
  it('renders the default idle shell', () => {
    const { container, getByText } = render(<App />);

    expect(container.querySelector('[data-testid="status-light"]')).toHaveAttribute('data-status', 'idle_unbound');
    expect(getByText('IDLE UNBOUND')).toBeInTheDocument();
  });

  it('renders a sound toggle control on the lower right side of the signal UI', () => {
    render(<App />);

    expect(
      screen.getByRole('button', {
        name: 'Mute status sounds'
      })
    ).toBeInTheDocument();
  });
});
